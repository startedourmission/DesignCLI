//! `.dxpkg` 포맷 — doc.json(구조) + 참조 표면 바이너리(네이티브 `.dxdoc`과 바이트 호환).
//!
//! 단일 파일 스냅샷 코덱. wasm `Editor`(브라우저), 데몬, CLI가 **모두 같은 코덱**을 쓰도록
//! 여기 한 곳에서만 정의한다(스냅샷 포맷 단일 진실원). 코어(model/tile)만 의존하므로
//! wasm32·native 양쪽에서 동일하게 컴파일된다(std::fs 무의존).
//!
//! 레이아웃(little-endian):
//!   "DXPKG\0"(6B) | version u32 | doc.json len u32 | [doc.json] |
//!   v1: surface 개수 u32 | (SurfaceId u64 | bytes len u32 | [Surface::to_bytes])*
//!   v2: surface 개수 u32 |
//!       (SurfaceId u64 | codec u8 | decoded len u32 | stored len u32 | [bytes])*

use dcli_model::Document;
use dcli_tile::{PixelStore, Surface, SurfaceId};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use std::io::{Read, Write};

const MAGIC: &[u8; 6] = b"DXPKG\0";
const VERSION: u32 = 2;
const CODEC_RAW: u8 = 0;
const CODEC_ZLIB: u8 = 1;

/// 문서를 `.dxpkg` 단일 파일 바이트로 직렬화(저장/스냅샷).
pub fn encode(doc: &Document) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());

    let json = doc.to_json().expect("문서 직렬화");
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(json.as_bytes());

    // 참조되는 표면만(orphan 제외) id 오름차순.
    let referenced = doc.referenced_surfaces();
    let surfaces: Vec<_> = doc
        .pixels()
        .iter_sorted()
        .filter(|(id, _)| referenced.contains(id))
        .collect();
    out.extend_from_slice(&(surfaces.len() as u32).to_le_bytes());
    // 표면별 (원시 길이, 압축 결과) — 네이티브는 rayon 병렬(PSD급 다MB 문서에서 직렬은
    // 수 초~십수 초; 데몬 스냅샷이 이 경로다). 순서 보존 → 출력 바이트는 직렬과 동일.
    let packed = pack_surfaces(&surfaces);
    for ((id, _), (raw_len, p)) in surfaces.iter().zip(packed) {
        out.extend_from_slice(&id.0.to_le_bytes());
        out.push(p.codec);
        out.extend_from_slice(&(raw_len as u32).to_le_bytes());
        out.extend_from_slice(&(p.bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&p.bytes);
    }
    out
}

#[cfg(not(target_arch = "wasm32"))]
fn pack_surfaces(surfaces: &[(SurfaceId, &Surface)]) -> Vec<(usize, PackedSurface)> {
    use rayon::prelude::*;
    surfaces
        .par_iter()
        .map(|(_, s)| {
            let bytes = s.to_bytes();
            (bytes.len(), compress_surface_bytes(&bytes))
        })
        .collect()
}

#[cfg(target_arch = "wasm32")]
fn pack_surfaces(surfaces: &[(SurfaceId, &Surface)]) -> Vec<(usize, PackedSurface)> {
    surfaces
        .iter()
        .map(|(_, s)| {
            let bytes = s.to_bytes();
            (bytes.len(), compress_surface_bytes(&bytes))
        })
        .collect()
}

struct PackedSurface {
    codec: u8,
    bytes: Vec<u8>,
}

fn compress_surface_bytes(bytes: &[u8]) -> PackedSurface {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::fast());
    if enc.write_all(bytes).is_ok() {
        if let Ok(compressed) = enc.finish() {
            if compressed.len() + 16 < bytes.len() {
                return PackedSurface {
                    codec: CODEC_ZLIB,
                    bytes: compressed,
                };
            }
        }
    }
    PackedSurface {
        codec: CODEC_RAW,
        bytes: bytes.to_vec(),
    }
}

/// `.dxpkg` 바이트에서 문서를 복원(열기).
pub fn decode(bytes: &[u8]) -> Result<Document, String> {
    let mut cur = 0usize;
    let take = |cur: &mut usize, n: usize| -> Result<&[u8], String> {
        if *cur + n > bytes.len() {
            return Err("dxpkg: 바이트 부족".into());
        }
        let s = &bytes[*cur..*cur + n];
        *cur += n;
        Ok(s)
    };
    let u32le = |cur: &mut usize| -> Result<u32, String> {
        Ok(u32::from_le_bytes(take(cur, 4)?.try_into().unwrap()))
    };

    if take(&mut cur, 6)? != MAGIC {
        return Err("dxpkg: magic 불일치".into());
    }
    let version = u32le(&mut cur)?;
    if version != 1 && version != VERSION {
        return Err(format!("dxpkg: 미지원 버전 {version}"));
    }

    let json_len = u32le(&mut cur)? as usize;
    let json = std::str::from_utf8(take(&mut cur, json_len)?)
        .map_err(|e| format!("dxpkg: doc.json utf8: {e}"))?;
    let mut doc = Document::from_json(json).map_err(|e| format!("dxpkg: doc.json 파싱: {e}"))?;

    let mut store = PixelStore::new();
    let count = u32le(&mut cur)?;
    for _ in 0..count {
        let id = SurfaceId(u64::from_le_bytes(take(&mut cur, 8)?.try_into().unwrap()));
        let sbytes = if version == 1 {
            let blen = u32le(&mut cur)? as usize;
            take(&mut cur, blen)?.to_vec()
        } else {
            let codec = take(&mut cur, 1)?[0];
            let decoded_len = u32le(&mut cur)? as usize;
            let stored_len = u32le(&mut cur)? as usize;
            let stored = take(&mut cur, stored_len)?;
            match codec {
                CODEC_RAW => {
                    if stored.len() != decoded_len {
                        return Err("dxpkg: raw 표면 길이 불일치".into());
                    }
                    stored.to_vec()
                }
                CODEC_ZLIB => {
                    let mut dec = ZlibDecoder::new(stored);
                    let mut out = Vec::with_capacity(decoded_len);
                    dec.read_to_end(&mut out)
                        .map_err(|e| format!("dxpkg: zlib 표면 디코드: {e}"))?;
                    if out.len() != decoded_len {
                        return Err("dxpkg: zlib 표면 길이 불일치".into());
                    }
                    out
                }
                other => return Err(format!("dxpkg: 알 수 없는 표면 codec {other}")),
            }
        };
        let surface = Surface::from_bytes(&sbytes).ok_or("dxpkg: 표면 디코드 실패")?;
        store.restore(id, surface);
    }
    doc.set_pixels(store);
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcli_color::BitDepth;

    #[test]
    fn empty_doc_roundtrip() {
        let doc = Document::new(10, 8, BitDepth::U8);
        let pkg = encode(&doc);
        let back = decode(&pkg).unwrap();
        assert_eq!(back.width, 10);
        assert_eq!(back.height, 8);
        assert_eq!(back.node_count(), 0);
    }

    #[test]
    fn bad_magic_rejected() {
        assert!(decode(b"NOPE..").is_err());
        assert!(decode(b"").is_err());
    }

    #[test]
    fn repeated_surface_payload_is_compressed() {
        use dcli_color::LinearPremul;
        use dcli_model::{History, Op};
        let mut hist = History::new(Document::new(512, 512, BitDepth::U8));
        let sid = hist.doc.add_surface(Surface::filled(
            512,
            512,
            LinearPremul::from_srgb8_straight(244, 235, 218, 255),
        ));
        hist.apply(Op::AddPaintLayer {
            name: "bg".into(),
            surface: sid,
            index: None,
            forced_id: None,
        })
        .unwrap();
        let raw_len = hist.doc.pixels().get(sid).unwrap().to_bytes().len();
        let pkg = encode(&hist.doc);
        assert!(
            pkg.len() < raw_len / 20,
            "압축 효과 부족: raw={raw_len} pkg={}",
            pkg.len()
        );
        let back = decode(&pkg).unwrap();
        assert_eq!(
            back.pixels().get(sid).unwrap().to_bytes(),
            hist.doc.pixels().get(sid).unwrap().to_bytes()
        );
    }
}
