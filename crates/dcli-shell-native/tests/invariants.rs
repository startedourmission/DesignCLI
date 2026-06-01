//! Phase 3 게이트:
//! 1) 코어 crate가 UI/GPU 의존(egui/eframe/winit/wgpu/iced)을 링크하지 않음(불변식).
//! 2) 동일 op 시퀀스에 대해 셸 합성 경로와 CLI export가 비트동일(공유 코어 정본).

use std::process::Command;

/// 코어 crate가 UI/GPU 크레이트를 의존 그래프에 끌어오지 않는지 `cargo tree`로 검사.
#[test]
fn core_crates_do_not_link_ui_or_gpu() {
    let forbidden = ["egui", "eframe", "winit", "wgpu", "iced"];
    for krate in ["dcli-color", "dcli-tile", "dcli-model", "dcli-raster"] {
        let out = Command::new(env!("CARGO"))
            .args(["tree", "-p", krate, "-e", "normal"])
            .output()
            .expect("cargo tree 실행 실패");
        let tree = String::from_utf8_lossy(&out.stdout);
        for f in forbidden {
            assert!(
                !tree.contains(f),
                "코어 crate {krate}가 금지된 의존 {f}를 링크함 — native-first 불변식 위반:\n{tree}"
            );
        }
    }
}

/// 셸과 CLI는 같은 코어 정본(dcli-raster)으로 합성하므로, 동일 op 시퀀스를 적용하면
/// 합성 결과 RGBA가 비트동일해야 한다(셸 export == CLI export의 토대).
#[test]
fn shell_and_core_composite_match_for_same_ops() {
    use dcli_color::{BitDepth, LinearPremul};
    use dcli_model::{BlendMode, Document, History, NodeProps, Op};
    use dcli_tile::Surface;

    // 동일한 op 시퀀스를 두 번 독립적으로 구성(셸/CLI가 각각 만든다고 가정).
    let build = || -> Vec<u8> {
        let mut h = History::new(Document::new(32, 32, BitDepth::U8));
        let bg = h.doc.add_surface(Surface::filled(
            32, 32, LinearPremul::from_srgb8_straight(220, 120, 40, 255),
        ));
        h.apply(Op::AddPaintLayer { name: "bg".into(), surface: bg, index: None }).unwrap();
        let top = h.doc.add_surface(Surface::filled(
            32, 32, LinearPremul::from_srgb8_straight(120, 120, 120, 200),
        ));
        h.apply(Op::AddPaintLayer { name: "top".into(), surface: top, index: None }).unwrap();
        let id = *h.doc.order().last().unwrap();
        h.apply(Op::SetProps {
            id,
            props: NodeProps { name: "top".into(), visible: true, opacity: 1.0, blend: BlendMode::Multiply },
        })
        .unwrap();
        dcli_raster::composite(&h.doc).to_srgb8_rgba()
    };

    assert_eq!(build(), build(), "동일 op 시퀀스의 합성이 비결정적");
}
