//! Document → PSD bytes (손수 인코딩 — 쓰기 크레이트 없음).
//!
//! 포맷: PSD v1(8BPS), 8bit, RGB 모드, 채널 4개(RGBA). 채널 데이터는 전부
//! RAW(compression 0)로 단순하게 기록한다. 모든 정수는 big-endian(PSD 규약).
//!
//! 각 레이어는 **트랜스폼 베이크**: 해당 레이어 하나만 넣은 임시 Document를
//! `dcli_raster::composite`(CPU 정본, 공개 API)로 캔버스 크기로 평탄화한 뒤
//! 불투명 bbox로 crop한 픽셀과 위치(rect)를 기록한다 — offset/scale/rotation이
//! 픽셀에 구워지고, blend/opacity/visible은 PSD 레이어 속성으로 따로 보존된다.

use dcli_model::{BlendMode, Document, Op};

/// 베이크가 끝난 레이어 한 장 — 인코더가 소비하는 중간 표현.
struct BakedLayer {
    name: String,
    /// PSD 4자 블렌드 키.
    blend_key: [u8; 4],
    /// 0=투명 ... 255=불투명.
    opacity: u8,
    /// true면 플래그 bit1(숨김) set.
    hidden: bool,
    /// (top, left, bottom, right). bottom/right는 **exclusive**(PSD rect 규약).
    rect: (i32, i32, i32, i32),
    /// R, G, B, A 평면(각 w×h 바이트, straight sRGB8).
    planes: [Vec<u8>; 4],
}

impl BakedLayer {
    fn width(&self) -> usize {
        (self.rect.3 - self.rect.1) as usize
    }
    fn height(&self) -> usize {
        (self.rect.2 - self.rect.0) as usize
    }
}

/// Document를 PSD 바이트로 인코딩한다.
///
/// 루트 order의 페인트 레이어만 기록한다(그룹 트리 export는 후속 Phase — 그룹
/// 내용은 composite image data 섹션에는 포함된다). 불투명 픽셀이 전혀 없는
/// 레이어는 기록할 rect가 없어 건너뛴다.
pub fn export_psd(doc: &Document) -> Vec<u8> {
    let baked = bake_layers(doc);

    let mut out = Vec::new();

    // ---- File Header (26바이트 고정) ----
    out.extend_from_slice(b"8BPS");
    out.extend_from_slice(&1u16.to_be_bytes()); // version 1 = PSD
    out.extend_from_slice(&[0u8; 6]); // reserved
    out.extend_from_slice(&4u16.to_be_bytes()); // 채널 수(RGBA)
    out.extend_from_slice(&doc.height.to_be_bytes());
    out.extend_from_slice(&doc.width.to_be_bytes());
    out.extend_from_slice(&8u16.to_be_bytes()); // 비트깊이 8
    out.extend_from_slice(&3u16.to_be_bytes()); // 컬러 모드 3 = RGB

    // ---- Color Mode Data (RGB는 내용 없음) ----
    out.extend_from_slice(&0u32.to_be_bytes());

    // ---- Image Resources (없음) ----
    out.extend_from_slice(&0u32.to_be_bytes());

    // ---- Layer and Mask Information ----
    let layer_info = encode_layer_info(&baked);
    // 섹션 길이 = layer info(길이필드 4 + 데이터) + global layer mask info(길이필드 4, 내용 0).
    let section_len = (4 + layer_info.len() + 4) as u32;
    out.extend_from_slice(&section_len.to_be_bytes());
    out.extend_from_slice(&(layer_info.len() as u32).to_be_bytes());
    out.extend_from_slice(&layer_info);
    out.extend_from_slice(&0u32.to_be_bytes()); // global layer mask info 없음

    // ---- Image Data (전체 합성 — 레이어를 못 읽는 뷰어 호환) ----
    out.extend_from_slice(&0u16.to_be_bytes()); // RAW
    let flat = dcli_raster::composite(doc).to_srgb8_rgba();
    for ch in 0..4 {
        out.extend(flat.iter().skip(ch).step_by(4)); // planar: RRR.. GGG.. BBB.. AAA..
    }

    out
}

/// 루트 order의 페인트 레이어들을 bottom-to-top으로 베이크한다.
fn bake_layers(doc: &Document) -> Vec<BakedLayer> {
    let mut out = Vec::new();
    for node in doc.iter_bottom_to_top() {
        // 픽셀 베이크: 페인트는 단독 임시문서, 그룹은 **자식 전체 평탄화 한 장**으로
        // (PSD 그룹 트리는 후속 — 평탄 레이어로라도 내용 보존이 우선).
        // blend/opacity/visible은 PSD 레이어 속성으로 기록하므로 베이크는 기하만 적용.
        let rgba = match &node.kind {
            dcli_model::NodeKind::Paint { surface } => {
                let Some(surface) = doc.pixels().get(*surface) else { continue };
                let mut tmp = Document::new(doc.width, doc.height, doc.bit_depth);
                tmp.blend_space = doc.blend_space; // 원본과 동일 합성 색공간 유지.
                let tsid = tmp.add_surface(surface.clone());
                let add = Op::AddPaintLayer {
                    name: node.name.clone(),
                    surface: tsid,
                    index: None,
                    forced_id: None,
                };
                if add.apply(&mut tmp).is_err() {
                    continue;
                }
                let tid = *tmp.order().last().expect("방금 추가한 노드");
                let t = tmp.get_mut(tid).expect("방금 추가한 노드");
                t.offset = node.offset;
                t.scale = node.scale;
                t.rotation = node.rotation;
                dcli_raster::composite(&tmp).to_srgb8_rgba()
            }
            dcli_model::NodeKind::Group { .. } => {
                // 원본 문서를 복제해 이 그룹만 표시 — 그룹 트랜스폼 포함 평탄화.
                // 그룹 자체 blend/opacity는 PSD 레이어 속성으로 기록되므로 중립화.
                let mut tmp = doc.clone();
                let order: Vec<_> = tmp.order().to_vec();
                for oid in order {
                    if oid != node.id {
                        if let Some(n) = tmp.get_mut(oid) {
                            n.visible = false;
                        }
                    }
                }
                if let Some(g) = tmp.get_mut(node.id) {
                    g.visible = true;
                    g.opacity = 1.0;
                    g.blend = dcli_model::BlendMode::Normal;
                }
                dcli_raster::composite(&tmp).to_srgb8_rgba()
            }
        };

        // 불투명(alpha>0) bbox 탐색 — 비면 기록할 픽셀이 없으므로 레이어 생략.
        let (w, h) = (doc.width as usize, doc.height as usize);
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0usize, 0usize);
        let mut any = false;
        for y in 0..h {
            for x in 0..w {
                if rgba[(y * w + x) * 4 + 3] != 0 {
                    any = true;
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        if !any {
            continue;
        }

        // bbox crop + 채널 평면 분리.
        let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut planes: [Vec<u8>; 4] = std::array::from_fn(|_| Vec::with_capacity(bw * bh));
        for (c, plane) in planes.iter_mut().enumerate() {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    plane.push(rgba[(y * w + x) * 4 + c]);
                }
            }
        }

        out.push(BakedLayer {
            name: node.name.clone(),
            blend_key: blend_key(node.blend),
            opacity: (node.opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
            hidden: !node.visible,
            rect: (y0 as i32, x0 as i32, (y1 + 1) as i32, (x1 + 1) as i32),
            planes,
        });
    }
    out
}

/// Layer Info 블록(레이어 카운트 + 레이어 레코드들 + 채널 이미지 데이터)을 인코딩한다.
/// 반환 길이는 짝수로 패딩된다(spec: rounded up to a multiple of 2).
fn encode_layer_info(baked: &[BakedLayer]) -> Vec<u8> {
    let mut info = Vec::new();

    // 레이어 카운트(i16). PSD 파일은 맨 아래 레이어가 첫 레코드 — 우리 bottom-to-top
    // 순서 그대로 기록하면 된다.
    info.extend_from_slice(&(baked.len() as i16).to_be_bytes());

    // ---- 레이어 레코드 ----
    for b in baked {
        let (top, left, bottom, right) = b.rect;
        info.extend_from_slice(&top.to_be_bytes());
        info.extend_from_slice(&left.to_be_bytes());
        info.extend_from_slice(&bottom.to_be_bytes());
        info.extend_from_slice(&right.to_be_bytes());

        // 채널 정보: id(i16) + 데이터 길이(u32, compression 2바이트 포함).
        info.extend_from_slice(&4u16.to_be_bytes());
        let channel_ids: [i16; 4] = [0, 1, 2, -1]; // R, G, B, 투명 마스크(alpha).
        for (id, plane) in channel_ids.iter().zip(b.planes.iter()) {
            info.extend_from_slice(&id.to_be_bytes());
            info.extend_from_slice(&((plane.len() + 2) as u32).to_be_bytes());
        }

        info.extend_from_slice(b"8BIM"); // 블렌드 모드 시그니처
        info.extend_from_slice(&b.blend_key);
        info.push(b.opacity);
        info.push(0); // clipping: 0 = base
        // 플래그: bit3(0x08) = "bit4 유효"(Photoshop 5+ 관행), bit1(0x02) = 숨김.
        // ※ Adobe 문서 표에는 bit1이 "visible"로 적혀 있으나 실제 파일에서는
        //   set = 숨김이다(import 쪽 주석 참조).
        info.push(0x08 | if b.hidden { 0x02 } else { 0x00 });
        info.push(0); // filler

        // extra data = 레이어 마스크(없음, 길이필드 4) + 블렌딩 레인지(없음, 길이필드 4)
        //              + pascal 이름(4바이트 배수 패딩).
        let pname = pascal_name_padded(&b.name);
        info.extend_from_slice(&((4 + 4 + pname.len()) as u32).to_be_bytes());
        info.extend_from_slice(&0u32.to_be_bytes()); // 레이어 마스크 데이터 없음
        info.extend_from_slice(&0u32.to_be_bytes()); // 블렌딩 레인지 없음
        info.extend_from_slice(&pname);
    }

    // ---- 채널 이미지 데이터(레코드와 같은 레이어/채널 순서) ----
    for b in baked {
        debug_assert_eq!(b.planes[0].len(), b.width() * b.height());
        for plane in &b.planes {
            info.extend_from_slice(&0u16.to_be_bytes()); // RAW
            info.extend_from_slice(plane);
        }
    }

    // 짝수 길이 패딩.
    if info.len() % 2 == 1 {
        info.push(0);
    }
    info
}

/// 본 엔진 BlendMode → PSD 4자 키. enum 7종 전부 1:1 매핑 존재.
fn blend_key(b: BlendMode) -> [u8; 4] {
    match b {
        BlendMode::Normal => *b"norm",
        BlendMode::Multiply => *b"mul ",
        BlendMode::Screen => *b"scrn",
        BlendMode::Darken => *b"dark",
        BlendMode::Lighten => *b"lite",
        BlendMode::Overlay => *b"over",
        BlendMode::Difference => *b"diff",
    }
}

/// Pascal string(길이 1바이트 + 본문)을 4바이트 배수로 0 패딩한다(PSD 규약).
/// 255바이트 초과 이름은 UTF-8 문자 경계에서 자른다.
fn pascal_name_padded(name: &str) -> Vec<u8> {
    let mut end = name.len().min(255);
    while end > 0 && !name.is_char_boundary(end) {
        end -= 1;
    }
    let bytes = &name.as_bytes()[..end];
    let mut out = Vec::with_capacity(bytes.len() + 4);
    out.push(bytes.len() as u8);
    out.extend_from_slice(bytes);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out
}
