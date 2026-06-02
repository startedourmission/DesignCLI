//! MCP tool 서버 — CLI verb ≡ MCP tool(cli-agent-interface).
//!
//! 모든 쓰기는 dcli-cli::dispatch(CLI와 공유 엔진)를 통과한다. read tool은 dto(CLI와
//! 공유 JSON 셰이프)로 응답한다. 에러 2분기(검증/MCP 스펙 SEP-1303): 입력 검증·op 실패·
//! batch 롤백은 isError:true + structured(self-correction용)로, 미지 tool/파싱 불가만
//! JSON-RPC error로 내려간다(rmcp가 후자를 처리).

use std::sync::Arc;

use dcli_cli::dispatch::{self, Action};
use dcli_cli::dto;
use dcli_color::BitDepth;
use dcli_model::{Document, NodeId};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use crate::session::{DocId, Workspace};
use crate::snapshot::snapshot_png;

// ---- 요청 파라미터 구조체(JsonSchema → inputSchema 자동 생성) ----

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocOpenReq {
    /// .dxdoc 폴더 경로.
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocCreateReq {
    pub path: String,
    #[serde(default = "default_dim")]
    pub w: u32,
    #[serde(default = "default_dim")]
    pub h: u32,
    /// 비트깊이: "u8"(감마 합성) | "u16" | "f32"(리니어 합성).
    #[serde(default = "default_depth")]
    pub depth: String,
}

fn default_dim() -> u32 {
    512
}
fn default_depth() -> String {
    "u8".to_string()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocRef {
    /// 서버 발급 문서 핸들(doc_open/doc_create가 반환).
    pub doc: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LayerGetReq {
    pub doc: String,
    /// 노드 id(layer_list의 id).
    pub id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BatchReq {
    pub doc: String,
    /// 적용할 Action 배열(트랜잭션 — 전부 성공 또는 전체 롤백).
    pub actions: Vec<Action>,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SnapshotReq {
    pub doc: String,
    /// 긴 변 최대 픽셀(미지정/0이면 원본 = export 비트와 동일). 토큰 절약용.
    #[serde(default)]
    pub max_dim: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportReq {
    pub doc: String,
    /// 출력 PNG 경로.
    pub out: String,
}

// ---- 서버 ----

#[derive(Clone)]
pub struct DesignServer {
    ws: Arc<Mutex<Workspace>>,
    // #[tool_handler] 매크로가 라우팅에 사용하지만 dead_code lint가 매크로 생성 코드를
    // 따라가지 못해 경고가 난다 — 실제로는 read된다.
    #[allow(dead_code)]
    tool_router: ToolRouter<DesignServer>,
}

impl Default for DesignServer {
    fn default() -> Self {
        Self::new()
    }
}

/// 검증/실행 실패 → isError:true + structured(self-correction).
fn tool_err(msg: impl Into<String>) -> CallToolResult {
    let msg = msg.into();
    let mut r = CallToolResult::error(vec![Content::text(msg.clone())]);
    r.structured_content = Some(json!({ "error": msg }));
    r
}

/// 성공 + 구조화 JSON(+ 하위호환용 text content 병기).
fn tool_ok(value: serde_json::Value) -> CallToolResult {
    let text = value.to_string();
    let mut r = CallToolResult::success(vec![Content::text(text)]);
    r.structured_content = Some(value);
    r
}

fn parse_depth(s: &str) -> Result<BitDepth, String> {
    match s {
        "u8" => Ok(BitDepth::U8),
        "u16" => Ok(BitDepth::U16),
        "f32" => Ok(BitDepth::F32),
        other => Err(format!("알 수 없는 비트깊이: {other} (u8|u16|f32)")),
    }
}

#[tool_router]
impl DesignServer {
    pub fn new() -> Self {
        Self {
            ws: Arc::new(Mutex::new(Workspace::new())),
            tool_router: Self::tool_router(),
        }
    }

    // ----- 문서 (인지/세션) -----

    #[tool(description = "디스크의 .dxdoc 문서 폴더를 세션으로 연다. 서버 발급 doc 핸들을 반환.")]
    async fn doc_open(&self, Parameters(req): Parameters<DocOpenReq>) -> CallToolResult {
        let mut ws = self.ws.lock().await;
        match ws.open(req.path.into()) {
            Ok(id) => {
                let s = ws.get(&id).unwrap();
                let mut info = dto::doc_info_json(&s.history.doc);
                info["doc"] = json!(id.0);
                info["missing_surfaces"] = json!(s.missing_surfaces());
                tool_ok(info)
            }
            Err(e) => tool_err(e.to_string()),
        }
    }

    #[tool(description = "새 문서를 생성해 디스크에 저장하고 세션으로 연다.")]
    async fn doc_create(&self, Parameters(req): Parameters<DocCreateReq>) -> CallToolResult {
        let depth = match parse_depth(&req.depth) {
            Ok(d) => d,
            Err(e) => return tool_err(e),
        };
        let doc = Document::new(req.w, req.h, depth);
        let mut ws = self.ws.lock().await;
        match ws.create(req.path.into(), doc) {
            Ok(id) => {
                let s = ws.get(&id).unwrap();
                let mut info = dto::doc_info_json(&s.history.doc);
                info["doc"] = json!(id.0);
                tool_ok(info)
            }
            Err(e) => tool_err(e.to_string()),
        }
    }

    #[tool(description = "문서 메타 정보(희소). 인지 루프 1단계.")]
    async fn doc_info(&self, Parameters(req): Parameters<DocRef>) -> CallToolResult {
        let ws = self.ws.lock().await;
        match ws.get(&DocId(req.doc.clone())) {
            Some(s) => {
                let mut info = dto::doc_info_json(&s.history.doc);
                info["doc"] = json!(req.doc);
                info["missing_surfaces"] = json!(s.missing_surfaces());
                tool_ok(info)
            }
            None => tool_err(format!("열린 문서 없음: {}", req.doc)),
        }
    }

    #[tool(description = "열린 문서 세션 목록.")]
    async fn doc_list(&self) -> CallToolResult {
        let ws = self.ws.lock().await;
        let docs: Vec<_> = ws
            .list()
            .into_iter()
            .map(|(id, path, layers)| json!({ "doc": id.0, "path": path.display().to_string(), "layers": layers }))
            .collect();
        tool_ok(json!({ "docs": docs }))
    }

    #[tool(description = "세션을 닫는다(저장 후). 락 해제.")]
    async fn doc_close(&self, Parameters(req): Parameters<DocRef>) -> CallToolResult {
        let mut ws = self.ws.lock().await;
        let id = DocId(req.doc.clone());
        let saved = match ws.get(&id) {
            Some(s) => s.save().is_ok(),
            None => return tool_err(format!("열린 문서 없음: {}", req.doc)),
        };
        ws.close(&id);
        tool_ok(json!({ "closed": true, "saved": saved }))
    }

    #[tool(description = "합성 결과를 PNG 이미지로 반환(에이전트가 결과를 본다). max_dim으로 다운스케일.")]
    async fn doc_snapshot(&self, Parameters(req): Parameters<SnapshotReq>) -> CallToolResult {
        let ws = self.ws.lock().await;
        let Some(s) = ws.get(&DocId(req.doc.clone())) else {
            return tool_err(format!("열린 문서 없음: {}", req.doc));
        };
        match snapshot_png(&s.history.doc, req.max_dim) {
            Ok((png, w, h, scaled)) => {
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
                let mut r = CallToolResult::success(vec![
                    Content::image(b64, "image/png"),
                    Content::text(format!("{w}x{h}{}", if scaled { " (scaled)" } else { "" })),
                ]);
                r.structured_content = Some(json!({ "w": w, "h": h, "scaled": scaled }));
                r
            }
            Err(e) => tool_err(e.to_string()),
        }
    }

    // ----- 레이어 (인지) -----

    #[tool(description = "레이어 목록(bottom-to-top). 인지 루프 2단계.")]
    async fn layer_list(&self, Parameters(req): Parameters<DocRef>) -> CallToolResult {
        let ws = self.ws.lock().await;
        match ws.get(&DocId(req.doc.clone())) {
            Some(s) => tool_ok(dto::layer_list_json(&s.history.doc)),
            None => tool_err(format!("열린 문서 없음: {}", req.doc)),
        }
    }

    #[tool(description = "단일 레이어 상세.")]
    async fn layer_get(&self, Parameters(req): Parameters<LayerGetReq>) -> CallToolResult {
        let ws = self.ws.lock().await;
        let Some(s) = ws.get(&DocId(req.doc.clone())) else {
            return tool_err(format!("열린 문서 없음: {}", req.doc));
        };
        match s.history.doc.get(NodeId(req.id)) {
            Some(node) => tool_ok(dto::node_json(node)),
            None => tool_err(format!("레이어 없음: n{}", req.id)),
        }
    }

    // ----- 쓰기 (batch가 주 경로) -----

    #[tool(description = "Action 배열을 트랜잭션으로 적용(주 쓰기 경로). 전부 성공 또는 전체 롤백. named binding(bind/ref)로 신규 노드 참조.")]
    async fn batch_apply(&self, Parameters(req): Parameters<BatchReq>) -> CallToolResult {
        let mut ws = self.ws.lock().await;
        let id = DocId(req.doc.clone());
        let Some(s) = ws.get_mut(&id) else {
            return tool_err(format!("열린 문서 없음: {}", req.doc));
        };
        let res = dispatch::apply_batch(&mut s.history, &req.actions, req.dry_run);
        if !res.ok {
            // batch 실패/롤백 → isError:true + 구조화 이슈(self-correction).
            let value = json!({
                "ok": false,
                "applied": res.applied,
                "aborted_at": res.aborted_at,
                "issues": res.issues,
            });
            let mut r = CallToolResult::error(vec![Content::text(value.to_string())]);
            r.structured_content = Some(value);
            return r;
        }
        // 성공: dry_run이 아니면 저장.
        let saved = if req.dry_run { false } else { s.save().is_ok() };
        let bindings: serde_json::Map<String, serde_json::Value> = res
            .bindings
            .iter()
            .map(|(k, v)| (k.clone(), json!({ "node": v.node, "surface": v.surface })))
            .collect();
        tool_ok(json!({
            "ok": true,
            "applied": res.applied,
            "bindings": bindings,
            "dry_run": req.dry_run,
            "saved": saved,
        }))
    }

    #[tool(description = "합성 결과를 PNG 파일로 export(디스크). snapshot과 달리 파일로 저장.")]
    async fn export_png(&self, Parameters(req): Parameters<ExportReq>) -> CallToolResult {
        let ws = self.ws.lock().await;
        let Some(s) = ws.get(&DocId(req.doc.clone())) else {
            return tool_err(format!("열린 문서 없음: {}", req.doc));
        };
        let surface = dcli_raster::composite(&s.history.doc);
        match dcli_cli::storage::export_png(std::path::Path::new(&req.out), &surface) {
            Ok(()) => tool_ok(json!({ "export": req.out, "w": surface.width(), "h": surface.height() })),
            Err(e) => tool_err(e.to_string()),
        }
    }

    // ----- 히스토리 -----

    #[tool(description = "마지막 편집(단발 op 또는 batch 전체)을 되돌린다.")]
    async fn history_undo(&self, Parameters(req): Parameters<DocRef>) -> CallToolResult {
        let mut ws = self.ws.lock().await;
        let Some(s) = ws.get_mut(&DocId(req.doc.clone())) else {
            return tool_err(format!("열린 문서 없음: {}", req.doc));
        };
        match s.history.undo() {
            Ok(undone) => {
                let saved = s.save().is_ok();
                tool_ok(json!({ "undone": undone, "layers": s.history.doc.node_count(), "saved": saved }))
            }
            Err(e) => tool_err(e.to_string()),
        }
    }

    #[tool(description = "되돌린 편집을 다시 적용한다.")]
    async fn history_redo(&self, Parameters(req): Parameters<DocRef>) -> CallToolResult {
        let mut ws = self.ws.lock().await;
        let Some(s) = ws.get_mut(&DocId(req.doc.clone())) else {
            return tool_err(format!("열린 문서 없음: {}", req.doc));
        };
        match s.history.redo() {
            Ok(redone) => {
                let saved = s.save().is_ok();
                tool_ok(json!({ "redone": redone, "layers": s.history.doc.node_count(), "saved": saved }))
            }
            Err(e) => tool_err(e.to_string()),
        }
    }
}

#[tool_handler]
impl ServerHandler for DesignServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "DesignCLI MCP 서버. 작업 흐름: doc_open/doc_create로 문서를 열어 doc 핸들을 \
             받고, batch_apply로 편집(주 쓰기 경로), doc_snapshot으로 결과를 본다. \
             신규 노드는 batch 안에서 bind 이름으로 참조한다(ID를 발명하지 말 것).",
        )
    }
}
