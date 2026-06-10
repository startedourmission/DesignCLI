//! 출력 헬퍼 — `--json`(stdout=데이터, 에이전트 친화) vs 사람 친화 텍스트.
//!
//! 규약(cli-agent-interface): --json이면 stdout에 구조화 JSON 한 줄, 에러는 항상
//! stderr, 성공 exit=0. dry-run이면 `applied:false`로 표시.

use dcli_cli::dto;
use dcli_model::{Document, Node};
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

    /// 데몬에서 받은 JSON 값을 그대로 출력(--json이면 raw, 아니면 human 라벨 + pretty).
    /// 서버 모드 읽기 명령(doc info / layer list)이 데몬 응답을 재모델링 없이 보여준다.
    pub fn raw_json_or(&self, value: &serde_json::Value, human_label: &str) {
        if self.json {
            println!("{value}");
        } else {
            println!("{human_label}:");
            println!(
                "{}",
                serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
            );
        }
    }

    pub fn error(&self, e: &anyhow::Error) {
        if self.json {
            eprintln!("{}", json!({ "error": e.to_string() }));
        } else {
            eprintln!("error: {e:#}");
        }
    }

    pub fn ok_target(&self, msg: &str, dry_run: bool, target: Option<&str>) {
        let mut value = json!({ "ok": true, "applied": !dry_run, "message": msg });
        if let Some(target) = target {
            value["target"] = json!(target);
        }
        self.put(
            value,
            &format!(
                "{}{msg}{}",
                if dry_run { "[dry-run] " } else { "" },
                target_suffix(target)
            ),
        );
    }

    pub fn doc_created_target(
        &self,
        path: &Path,
        doc: &Document,
        dry_run: bool,
        target: Option<&str>,
    ) {
        let mut value = json!({
            "doc": path.display().to_string(),
            "w": doc.width, "h": doc.height,
            "depth": format!("{:?}", doc.bit_depth),
            "blend_space": format!("{:?}", doc.blend_space),
            "applied": !dry_run,
        });
        if let Some(target) = target {
            value["target"] = json!(target);
        }
        self.put(
            value,
            &format!(
                "{}문서 생성: {} ({}x{}, {:?}, {:?} 합성){}",
                if dry_run { "[dry-run] " } else { "" },
                path.display(),
                doc.width,
                doc.height,
                doc.bit_depth,
                doc.blend_space,
                target_suffix(target)
            ),
        );
    }

    pub fn doc_info(&self, doc: &Document) {
        self.put(
            dto::doc_info_json(doc),
            &format!(
                "{}x{}, {:?}, {:?} 합성, 레이어 {}개",
                doc.width,
                doc.height,
                doc.bit_depth,
                doc.blend_space,
                doc.node_count()
            ),
        );
    }

    pub fn layer_added_target(
        &self,
        id: dcli_model::NodeId,
        name: &str,
        sid: SurfaceId,
        dry_run: bool,
        target: Option<&str>,
    ) {
        let mut value = json!({
            "layer": id.0, "name": name, "surface": sid.0, "applied": !dry_run,
        });
        if let Some(target) = target {
            value["target"] = json!(target);
        }
        self.put(
            value,
            &format!(
                "{}레이어 추가: n{} \"{}\" (표면 {}){}",
                if dry_run { "[dry-run] " } else { "" },
                id.0,
                name,
                sid,
                target_suffix(target)
            ),
        );
    }

    pub fn layer_list(&self, doc: &Document) {
        if self.json {
            println!("{}", dto::layer_list_json(doc));
        } else {
            println!("레이어 (아래→위):");
            for (i, node) in doc.iter_bottom_to_top().enumerate() {
                println!("  [{i}] {}", dto::node_human(node));
            }
        }
    }

    pub fn layer_get(&self, doc: &Document, node: &Node) {
        self.put(dto::node_json(doc, node), &dto::node_human(node));
    }

    pub fn exported_target(&self, out: &Path, w: u32, h: u32, dry_run: bool, target: Option<&str>) {
        let mut value =
            json!({ "export": out.display().to_string(), "w": w, "h": h, "applied": !dry_run });
        if let Some(target) = target {
            value["target"] = json!(target);
        }
        self.put(
            value,
            &format!(
                "{}export: {} ({}x{}){}",
                if dry_run { "[dry-run] " } else { "" },
                out.display(),
                w,
                h,
                target_suffix(target)
            ),
        );
    }
}

fn target_suffix(target: Option<&str>) -> String {
    match target {
        Some("live") => " (라이브 적용)".into(),
        Some("disk") => " (디스크 저장)".into(),
        Some(other) => format!(" ({other})"),
        None => String::new(),
    }
}
