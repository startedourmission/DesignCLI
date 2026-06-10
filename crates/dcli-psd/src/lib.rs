//! PSD ↔ Document 변환 (psd-compat).
//!
//! - **import**: `psd` 크레이트(read 전용)로 PSD 바이트를 파싱해 래스터 레이어를
//!   본 엔진 레이어로 변환한다. 레이어 rect 크기의 `Surface` + `offset`=rect 위치,
//!   name/opacity/visible 보존, 블렌드는 매핑 가능한 7종만(나머지 normal 폴백).
//! - **export**: 손수 인코딩(쓰기 크레이트 없음). PSD v1, 8bit RGBA, 채널 RAW
//!   (compression 0). 각 레이어는 **트랜스폼 베이크** — 해당 레이어 하나만 넣은
//!   임시 Document를 `dcli_raster::composite`(CPU 정본)로 평탄화한 뒤 불투명
//!   bbox로 crop한 픽셀+위치를 기록한다. 파일 끝 composite image data 섹션에는
//!   전체 합성을 넣는다(레이어를 못 읽는 뷰어 호환).
//!
//! CLI/MCP 배선은 소비자 몫 — 이 crate는 lib API(`import_psd`/`export_psd`)만 제공.

#![forbid(unsafe_code)]

mod export;
mod import;

pub use export::export_psd;
pub use import::import_psd;

/// PSD 변환 실패 사유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsdConvertError {
    /// PSD 바이트 파싱 실패(psd 크레이트 오류 메시지 보존).
    Parse(String),
    /// 문서 op 적용 실패(이론상 발생하지 않음 — 방어적 전파).
    Op(String),
}

impl std::fmt::Display for PsdConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PsdConvertError::Parse(msg) => write!(f, "PSD 파싱 실패: {msg}"),
            PsdConvertError::Op(msg) => write!(f, "문서 구성 실패: {msg}"),
        }
    }
}

impl std::error::Error for PsdConvertError {}

#[cfg(test)]
mod tests {
    use super::*;
    use dcli_color::BitDepth;
    use dcli_model::{BlendMode, Document, NodeId, Op};
    use dcli_raster::shapes;
    use dcli_tile::Surface;

    /// 표면을 등록하고 페인트 레이어를 맨 위에 추가한 뒤 NodeId를 돌려준다.
    fn add_layer(doc: &mut Document, name: &str, s: Surface) -> NodeId {
        let sid = doc.add_surface(s);
        Op::AddPaintLayer { name: name.into(), surface: sid, index: None, forced_id: None }
            .apply(doc)
            .unwrap();
        *doc.order().last().unwrap()
    }

    /// 도형을 그린 테스트 문서: 배경 + 오프셋·multiply 사각형 + 트랜스폼 타원 + 숨김 선.
    fn draw_doc() -> Document {
        let mut doc = Document::new(64, 48, BitDepth::U8);

        // 배경: 전체를 채운 불투명 사각형.
        let mut bg = Surface::new(64, 48);
        shapes::fill_rect(&mut bg, 0.0, 0.0, 64.0, 48.0, [30, 40, 50, 255]);
        add_layer(&mut doc, "Background", bg);

        // 빨간 사각형: 작은 표면 + offset 이동 + multiply + 80% 불투명.
        let mut s1 = Surface::new(20, 16);
        shapes::fill_rect(&mut s1, 2.0, 2.0, 16.0, 12.0, [220, 40, 30, 255]);
        let id = add_layer(&mut doc, "Red Rect", s1);
        let n = doc.get_mut(id).unwrap();
        n.offset = (5, 6);
        n.blend = BlendMode::Multiply;
        n.opacity = 0.8;

        // 녹색 타원: 비파괴 스케일·회전 → export 시 베이크되어야 한다.
        let mut s2 = Surface::new(64, 48);
        shapes::fill_ellipse(&mut s2, 40.0, 24.0, 10.0, 7.0, [40, 200, 90, 255]);
        let id = add_layer(&mut doc, "Ellipse", s2);
        let n = doc.get_mut(id).unwrap();
        n.scale = (1.5, 1.0);
        n.rotation = 20.0;

        // 숨김 선: visible=false 플래그 round-trip 검증용.
        let mut s3 = Surface::new(64, 48);
        shapes::stroke_line(&mut s3, 0.0, 0.0, 63.0, 47.0, 3.0, [255, 255, 0, 255]);
        let id = add_layer(&mut doc, "Hidden Line", s3);
        doc.get_mut(id).unwrap().visible = false;

        doc
    }

    #[test]
    fn roundtrip_layer_count_names_props() {
        let doc = draw_doc();
        let bytes = export_psd(&doc);
        let back = import_psd(&bytes).unwrap();

        assert_eq!(back.node_count(), 4, "레이어 수 보존");
        let names: Vec<&str> = back.iter_bottom_to_top().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["Background", "Red Rect", "Ellipse", "Hidden Line"]);

        let nodes: Vec<_> = back.iter_bottom_to_top().collect();
        assert_eq!(nodes[1].blend, BlendMode::Multiply, "multiply 키 round-trip");
        assert!((nodes[1].opacity - 0.8).abs() < 1.0 / 255.0, "opacity u8 양자화 내 보존");
        assert!(nodes[0].visible && nodes[1].visible && nodes[2].visible);
        assert!(!nodes[3].visible, "숨김 플래그 round-trip");
        // 베이크된 레이어는 불투명 bbox 위치가 offset으로 들어온다.
        assert_eq!(nodes[1].offset, (7, 8), "offset(5,6)+표면 내 (2,2) = bbox (7,8)");
    }

    #[test]
    fn roundtrip_composite_pixels_within_tolerance() {
        let doc = draw_doc();
        let back = import_psd(&export_psd(&doc)).unwrap();

        let a = dcli_raster::composite(&doc).to_srgb8_rgba();
        let b = dcli_raster::composite(&back).to_srgb8_rgba();
        assert_eq!(a.len(), b.len());
        let mut max_diff = 0i32;
        for (pa, pb) in a.iter().zip(b.iter()) {
            max_diff = max_diff.max((*pa as i32 - *pb as i32).abs());
        }
        assert!(max_diff <= 2, "합성 픽셀 최대 오차 {max_diff} > 2");
    }

    #[test]
    fn export_is_valid_psd_for_reader() {
        // psd 크레이트가 헤더/합성 섹션을 직접 읽을 수 있어야 한다(타 뷰어 호환 프록시).
        let doc = draw_doc();
        let bytes = export_psd(&doc);
        let psd = psd::Psd::from_bytes(&bytes).unwrap();
        assert_eq!(psd.width(), 64);
        assert_eq!(psd.height(), 48);
        assert_eq!(psd.layers().len(), 4);
        // composite image data 섹션 = 전체 합성과 ±2 일치.
        let composite = dcli_raster::composite(&doc).to_srgb8_rgba();
        let flat = psd.rgba();
        assert_eq!(flat.len(), composite.len());
        for (pa, pb) in composite.iter().zip(flat.iter()) {
            assert!((*pa as i32 - *pb as i32).abs() <= 2);
        }
    }

    #[test]
    fn empty_document_roundtrips() {
        let doc = Document::new(8, 8, BitDepth::U8);
        let back = import_psd(&export_psd(&doc)).unwrap();
        assert_eq!(back.node_count(), 0);
        assert_eq!((back.width, back.height), (8, 8));
    }

    #[test]
    fn fully_transparent_layer_is_skipped_on_export() {
        // 불투명 bbox가 비는 레이어는 기록할 픽셀이 없어 export에서 빠진다.
        let mut doc = Document::new(8, 8, BitDepth::U8);
        add_layer(&mut doc, "Empty", Surface::new(8, 8));
        let mut s = Surface::new(8, 8);
        shapes::fill_rect(&mut s, 1.0, 1.0, 4.0, 4.0, [255, 0, 255, 255]);
        add_layer(&mut doc, "Dot", s);
        let back = import_psd(&export_psd(&doc)).unwrap();
        assert_eq!(back.node_count(), 1);
        assert_eq!(back.iter_bottom_to_top().next().unwrap().name, "Dot");
    }
}
