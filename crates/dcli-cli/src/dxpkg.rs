//! `.dxpkg` 포맷 — doc.json(구조) + 참조 표면 바이너리(네이티브 `.dxdoc`과 바이트 호환).
//!
//! 단일 파일 스냅샷 코덱. wasm `Editor`(브라우저), 데몬, CLI가 **모두 같은 코덱**을 쓰도록
//! 여기 한 곳에서만 정의한다(스냅샷 포맷 단일 진실원). 코어(model/tile)만 의존하므로
//! wasm32·native 양쪽에서 동일하게 컴파일된다(std::fs 무의존).
//!
//! 레이아웃(little-endian):
//!   "DXPKG\0"(6B) | version u32 | doc.json len u32 | [doc.json] |
//!   surface 개수 u32 | (SurfaceId u64 | bytes len u32 | [Surface::to_bytes])*

use dcli_model::Document;
use dcli_tile::{PixelStore, Surface, SurfaceId};

const MAGIC: &[u8; 6] = b"DXPKG\0";
const VERSION: u32 = 1;

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
    for (id, surface) in surfaces {
        out.extend_from_slice(&id.0.to_le_bytes());
        let bytes = surface.to_bytes();
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&bytes);
    }
    out
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
    if version != VERSION {
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
        let blen = u32le(&mut cur)? as usize;
        let sbytes = take(&mut cur, blen)?;
        let surface = Surface::from_bytes(sbytes).ok_or("dxpkg: 표면 디코드 실패")?;
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
}
