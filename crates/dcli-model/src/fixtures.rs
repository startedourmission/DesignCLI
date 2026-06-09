//! 테스트/스파이크용 표준 장면. 테스트와 example이 동일 입력을 공유한다.

use crate::op::add_paint_with_surface;
use crate::{BlendMode, Document, History, NodeProps};
use dcli_color::{BitDepth, LinearPremul};
use dcli_tile::Surface;

/// 가로 그라디언트 배경 (좌→우 색 보간). 블렌드가 배경 그라디언트를 통과해
/// 보이는지 검증하기 위함 — 균일 색 블록이면 블렌드 차이를 못 본다.
fn gradient(w: u32, h: u32, left: (u8, u8, u8), right: (u8, u8, u8)) -> Surface {
    let mut s = Surface::new(w, h);
    for x in 0..w {
        let t = x as f32 / (w - 1).max(1) as f32;
        let lerp = |a: u8, b: u8| -> u8 { (a as f32 * (1.0 - t) + b as f32 * t).round() as u8 };
        let c = LinearPremul::from_srgb8_straight(
            lerp(left.0, right.0),
            lerp(left.1, right.1),
            lerp(left.2, right.2),
            255,
        );
        for y in 0..h {
            s.set(x, y, c);
        }
    }
    s
}

fn top_half(w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Surface {
    let mut s = Surface::new(w, h);
    let c = LinearPremul::from_srgb8_straight(r, g, b, a);
    for y in 0..h / 2 {
        for x in 0..w {
            s.set(x, y, c);
        }
    }
    s
}

fn bottom_half(w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Surface {
    let mut s = Surface::new(w, h);
    let c = LinearPremul::from_srgb8_straight(r, g, b, a);
    for y in h / 2..h {
        for x in 0..w {
            s.set(x, y, c);
        }
    }
    s
}

/// 한 노드를 추가하고 blend/opacity 속성을 op으로 설정하는 헬퍼.
fn add_layer(h: &mut History, name: &str, surface: Surface, blend: BlendMode, opacity: f32) {
    let op = add_paint_with_surface(&mut h.doc, name, surface, None);
    h.apply(op).unwrap();
    let id = *h.doc.order().last().unwrap();
    h.apply(crate::Op::SetProps {
        id,
        props: NodeProps { name: name.to_string(), visible: true, opacity, blend, offset: (0, 0) },
    })
    .unwrap();
}

/// 표준 스파이크 장면: 그라디언트 배경 + 위쪽 Multiply + 아래쪽 Screen.
///
/// 배경이 좌(따뜻)→우(차가움) 그라디언트라 블렌드가 변하는 배경을 통과하며,
/// 블렌드 레이어를 위/아래로 분리해 Multiply(어두워짐)·Screen(밝아짐) 효과를
/// 한 화면에서 동시에 본다. 감마/리니어 경로 차이가 그라디언트 중간대에서 도드라진다.
/// `depth`로 감마(U8)/리니어(F32) 경로를 선택한다.
///
/// 이제 모든 편집이 op을 통과한다(Phase 1) — fixture조차 event-sourced 경로로 만든다.
pub fn spike_scene(depth: BitDepth) -> Document {
    let (w, h) = (128u32, 96u32);
    let mut hist = History::new(Document::new(w, h, depth));
    add_layer(&mut hist, "bg", gradient(w, h, (235, 130, 40), (30, 170, 205)), BlendMode::Normal, 1.0);
    add_layer(&mut hist, "mul", top_half(w, h, 150, 60, 200, 200), BlendMode::Multiply, 1.0);
    add_layer(&mut hist, "scr", bottom_half(w, h, 230, 210, 60, 200), BlendMode::Screen, 1.0);
    hist.doc
}
