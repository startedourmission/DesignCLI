//! PSD bytes → Document (psd 크레이트 read 전용).

use dcli_color::{BitDepth, LinearPremul};
use dcli_model::{BlendMode, Document, Op};
use dcli_tile::Surface;
use psd::Psd;

use crate::PsdConvertError;

/// PSD 바이트를 파싱해 Document로 변환한다.
///
/// - 래스터 레이어만 가져온다(그룹 트리는 평탄화 — 후속 Phase).
/// - 픽셀: RGBA8 → 레이어 rect 크기의 `Surface`, 노드 `offset` = rect 위치.
/// - name/opacity/visible 보존, 블렌드는 매핑 가능한 7종만(나머지 normal 폴백).
/// - 문서는 8bit(PSD v1 기준) → 감마 블렌드 공간(gamma-vs-linear-landmine).
pub fn import_psd(bytes: &[u8]) -> Result<Document, PsdConvertError> {
    let psd = Psd::from_bytes(bytes).map_err(|e| PsdConvertError::Parse(e.to_string()))?;
    let (pw, ph) = (psd.width(), psd.height());
    let mut doc = Document::new(pw, ph, BitDepth::U8);

    // psd 크레이트는 layers()[0]이 맨 위 레이어 → rev()로 bottom-to-top 순회하며
    // 맨 위에 차례로 append하면 본 엔진 순서(인덱스 0 = 맨 아래)와 일치한다.
    for layer in psd.layers().iter().rev() {
        let w = layer.width() as u32;
        let h = layer.height() as u32;
        if w == 0 || h == 0 {
            continue; // 픽셀 없는 퇴화 레이어는 건너뜀.
        }
        let (left, top) = (layer.layer_left(), layer.layer_top());

        // layer.rgba()는 **캔버스 크기**(pw×ph×4) 버퍼에 레이어를 제 위치에 박아
        // 돌려준다 → rect 영역만 잘라 rect 크기 표면으로 옮긴다.
        let rgba = layer.rgba();
        let mut surf = Surface::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let cx = left + x as i32;
                let cy = top + y as i32;
                if cx < 0 || cy < 0 || cx >= pw as i32 || cy >= ph as i32 {
                    continue; // 캔버스 밖 부분은 투명 유지(psd 크레이트도 버린다).
                }
                let i = ((cy as u32 * pw + cx as u32) * 4) as usize;
                surf.set(
                    x,
                    y,
                    LinearPremul::from_srgb8_straight(rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]),
                );
            }
        }

        let sid = doc.add_surface(surf);
        Op::AddPaintLayer {
            name: layer.name().to_string(),
            surface: sid,
            index: None, // 맨 위에 추가.
            forced_id: None,
        }
        .apply(&mut doc)
        .map_err(|e| PsdConvertError::Op(e.to_string()))?;

        let id = *doc.order().last().expect("방금 추가한 노드");
        let node = doc.get_mut(id).expect("방금 추가한 노드");
        node.offset = (left, top);
        node.opacity = f32::from(layer.opacity()) / 255.0;
        // ★주의★ psd 크레이트의 visible()은 플래그 bit1을 그대로 돌려주지만, 실제
        // Photoshop 파일에서 bit1 set = **숨김**이다 → 의미를 뒤집어 해석한다.
        node.visible = !layer.visible();
        node.blend = blend_from_psd_discriminant(layer.blend_mode() as u8);
    }

    Ok(doc)
}

/// psd 크레이트 BlendMode → 본 엔진 BlendMode.
///
/// psd 크레이트는 BlendMode 타입을 재export하지 않아(비공개 모듈) 타입을 명명할 수
/// 없다 → C-like enum 판별값 캐스트로 비교한다 (PassThrough=0, Normal=1, Darken=3,
/// Multiply=4, Lighten=8, Screen=9, Overlay=13, Difference=20).
fn blend_from_psd_discriminant(disc: u8) -> BlendMode {
    match disc {
        3 => BlendMode::Darken,
        4 => BlendMode::Multiply,
        8 => BlendMode::Lighten,
        9 => BlendMode::Screen,
        13 => BlendMode::Overlay,
        20 => BlendMode::Difference,
        // norm 포함, 매핑 불가 모드는 normal 폴백.
        _ => BlendMode::Normal,
    }
}
