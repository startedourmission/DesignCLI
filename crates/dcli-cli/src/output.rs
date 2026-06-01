//! 출력 헬퍼 — `--json`(stdout=데이터, 에이전트 친화) vs 사람 친화 텍스트.
//!
//! 규약(cli-agent-interface): --json이면 stdout에 구조화 JSON 한 줄, 에러는 항상
//! stderr, 성공 exit=0. dry-run이면 `applied:false`로 표시.

use dcli_model::{Document, Node, NodeKind};
use dcli_tile::SurfaceId;
use serde_json::json;
use std::path::Path;

pub struct Emitter {
    json: bool,
}

impl Emitter {
    pub fn new(json: bool) -> Self {
        Self { json }
    }

    fn put(&self, value: serde_json::Value, human: &str) {
        if self.json {
            println!("{}", value);
        } else {
            println!("{human}");
        }
    }

    pub fn error(&self, e: &anyhow::Error) {
        if self.json {
            eprintln!("{}", json!({ "error": e.to_string() }));
        } else {
            eprintln!("error: {e:#}");
        }
    }

    pub fn ok(&self, msg: &str, dry_run: bool) {
        self.put(
            json!({ "ok": true, "applied": !dry_run, "message": msg }),
            &format!("{}{msg}", if dry_run { "[dry-run] " } else { "" }),
        );
    }

    pub fn doc_created(&self, path: &Path, doc: &Document, dry_run: bool) {
        self.put(
            json!({
                "doc": path.display().to_string(),
                "w": doc.width, "h": doc.height,
                "depth": format!("{:?}", doc.bit_depth),
                "blend_space": format!("{:?}", doc.blend_space),
                "applied": !dry_run,
            }),
            &format!(
                "{}문서 생성: {} ({}x{}, {:?}, {:?} 합성)",
                if dry_run { "[dry-run] " } else { "" },
                path.display(), doc.width, doc.height, doc.bit_depth, doc.blend_space
            ),
        );
    }

    pub fn doc_info(&self, doc: &Document) {
        self.put(
            json!({
                "w": doc.width, "h": doc.height,
                "depth": format!("{:?}", doc.bit_depth),
                "blend_space": format!("{:?}", doc.blend_space),
                "layers": doc.node_count(),
            }),
            &format!(
                "{}x{}, {:?}, {:?} 합성, 레이어 {}개",
                doc.width, doc.height, doc.bit_depth, doc.blend_space, doc.node_count()
            ),
        );
    }

    pub fn layer_added(&self, id: dcli_model::NodeId, name: &str, sid: SurfaceId, dry_run: bool) {
        self.put(
            json!({
                "layer": id.0, "name": name, "surface": sid.0, "applied": !dry_run,
            }),
            &format!(
                "{}레이어 추가: n{} \"{}\" (표면 {})",
                if dry_run { "[dry-run] " } else { "" },
                id.0, name, sid
            ),
        );
    }

    pub fn layer_list(&self, doc: &Document) {
        if self.json {
            let arr: Vec<_> = doc
                .iter_bottom_to_top()
                .map(node_json)
                .collect();
            println!("{}", json!({ "layers": arr }));
        } else {
            println!("레이어 (아래→위):");
            for (i, node) in doc.iter_bottom_to_top().enumerate() {
                println!("  [{i}] {}", node_human(node));
            }
        }
    }

    pub fn layer_get(&self, node: &Node) {
        self.put(node_json(node), &node_human(node));
    }

    pub fn exported(&self, out: &Path, w: u32, h: u32, dry_run: bool) {
        self.put(
            json!({ "export": out.display().to_string(), "w": w, "h": h, "applied": !dry_run }),
            &format!(
                "{}export: {} ({}x{})",
                if dry_run { "[dry-run] " } else { "" },
                out.display(), w, h
            ),
        );
    }
}

fn node_kind_str(node: &Node) -> &'static str {
    match node.kind {
        NodeKind::Paint { .. } => "paint",
        NodeKind::Group => "group",
    }
}

fn node_json(node: &Node) -> serde_json::Value {
    json!({
        "id": node.id.0,
        "name": node.name,
        "kind": node_kind_str(node),
        "visible": node.visible,
        "opacity": node.opacity,
        "blend": format!("{:?}", node.blend),
        "surface": node.surface_id().map(|s| s.0),
    })
}

fn node_human(node: &Node) -> String {
    format!(
        "n{} \"{}\" [{}] {} opacity={:.2} {:?}",
        node.id.0,
        node.name,
        node_kind_str(node),
        if node.visible { "visible" } else { "hidden" },
        node.opacity,
        node.blend,
    )
}
