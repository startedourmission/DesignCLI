//! HTTP/WS 라우트 — 데몬의 공개 API. 쓰기는 전부 `dispatch::apply_batch`(또는 History
//! undo/redo) 단일 경로를 거치고 seq를 올린 뒤 broadcast한다.

use crate::state::{AppState, DocState, LiveMsg};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use base64::Engine;
use dcli_cli::dispatch::{self, Action};
use dcli_cli::{dto, dxpkg};
use dcli_color::BitDepth;
use dcli_model::{Document, History};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

type Shared = State<Arc<AppState>>;

/// POST /doc/:id/create?w&h&depth — 새 문서 등록.
#[derive(Deserialize)]
pub struct CreateParams {
    #[serde(default = "default_w")]
    w: u32,
    #[serde(default = "default_h")]
    h: u32,
    #[serde(default = "default_depth")]
    depth: String,
}
fn default_w() -> u32 {
    512
}
fn default_h() -> u32 {
    512
}
fn default_depth() -> String {
    "u8".into()
}

fn parse_depth(s: &str) -> Result<BitDepth, String> {
    match s {
        "u8" => Ok(BitDepth::U8),
        "u16" => Ok(BitDepth::U16),
        "f32" => Ok(BitDepth::F32),
        other => Err(format!("알 수 없는 비트깊이: {other} (u8|u16|f32)")),
    }
}

pub async fn create_doc(
    State(app): Shared,
    Path(id): Path<String>,
    Query(p): Query<CreateParams>,
) -> impl IntoResponse {
    let depth = match parse_depth(&p.depth) {
        Ok(d) => d,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };
    let mut docs = app.docs.lock().unwrap();
    if docs.contains_key(&id) {
        return (StatusCode::CONFLICT, format!("문서 '{id}' 이미 존재")).into_response();
    }
    let doc = Document::new(p.w, p.h, depth);
    docs.insert(id.clone(), DocState::new(History::new(doc)));
    (StatusCode::CREATED, Json(json!({ "id": id, "w": p.w, "h": p.h, "depth": p.depth, "seq": 0 })))
        .into_response()
}

/// GET /doc — 열린 문서 id 목록.
pub async fn list_docs(State(app): Shared) -> impl IntoResponse {
    let docs = app.docs.lock().unwrap();
    let ids: Vec<&String> = docs.keys().collect();
    Json(json!({ "docs": ids })).into_response()
}

/// POST /doc/:id/apply — Action 배열 적용(CLI·웹 공통 쓰기 경로).
pub async fn apply(
    State(app): Shared,
    Path(id): Path<String>,
    Json(actions): Json<Vec<Action>>,
) -> impl IntoResponse {
    let mut docs = app.docs.lock().unwrap();
    let Some(ds) = docs.get_mut(&id) else {
        return doc_not_found(&id);
    };
    let res = dispatch::apply_batch(&mut ds.hist, &actions, false);
    if res.ok {
        ds.seq += 1;
        // 구독자 없으면 send가 Err지만 무시(브로드캐스트 의미상 정상).
        let _ = ds.tx.send(LiveMsg::Ops { seq: ds.seq, actions });
    }
    // BatchResult는 Serialize라 직접 직렬화(dispatch가 단일 진실원).
    (StatusCode::OK, Json(json!({ "seq": ds.seq, "result": serde_json::to_value(&res).unwrap() })))
        .into_response()
}

/// POST /doc/:id/undo
pub async fn undo(State(app): Shared, Path(id): Path<String>) -> impl IntoResponse {
    let mut docs = app.docs.lock().unwrap();
    let Some(ds) = docs.get_mut(&id) else {
        return doc_not_found(&id);
    };
    match ds.hist.undo() {
        Ok(true) => {
            ds.seq += 1;
            let _ = ds.tx.send(LiveMsg::Undo { seq: ds.seq });
            Json(json!({ "ok": true, "changed": true, "seq": ds.seq })).into_response()
        }
        Ok(false) => Json(json!({ "ok": true, "changed": false, "seq": ds.seq })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// POST /doc/:id/redo
pub async fn redo(State(app): Shared, Path(id): Path<String>) -> impl IntoResponse {
    let mut docs = app.docs.lock().unwrap();
    let Some(ds) = docs.get_mut(&id) else {
        return doc_not_found(&id);
    };
    match ds.hist.redo() {
        Ok(true) => {
            ds.seq += 1;
            let _ = ds.tx.send(LiveMsg::Redo { seq: ds.seq });
            Json(json!({ "ok": true, "changed": true, "seq": ds.seq })).into_response()
        }
        Ok(false) => Json(json!({ "ok": true, "changed": false, "seq": ds.seq })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// GET /doc/:id/snapshot — {seq, dxpkg_base64}. 웹 초기화용.
pub async fn snapshot(State(app): Shared, Path(id): Path<String>) -> impl IntoResponse {
    let docs = app.docs.lock().unwrap();
    let Some(ds) = docs.get(&id) else {
        return doc_not_found(&id);
    };
    let bytes = dxpkg::encode(&ds.hist.doc);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Json(json!({ "seq": ds.seq, "dxpkg_base64": b64 })).into_response()
}

/// GET /doc/:id/export.png — 합성 결과를 8bit RGBA PNG로 응답(디스크 export와 동일 인코딩).
/// export 영역 선택: ?frame=<이름|id> 또는 ?region=x,y,w,h (없으면 문서 전체).
#[derive(Deserialize, Default)]
pub struct ExportParams {
    pub frame: Option<String>,
    pub region: Option<String>,
}

pub async fn export_png(
    State(app): Shared,
    Path(id): Path<String>,
    Query(p): Query<ExportParams>,
) -> impl IntoResponse {
    let docs = app.docs.lock().unwrap();
    let Some(ds) = docs.get(&id) else {
        return doc_not_found(&id);
    };
    let doc = &ds.hist.doc;
    let surface = if let Some(key) = &p.frame {
        // Frame 단위 export — 무한 작업영역의 캔버스.
        let Some(f) = doc.find_frame(key) else {
            return (StatusCode::NOT_FOUND, format!("frame '{key}' 없음")).into_response();
        };
        dcli_raster::composite_region(doc, f.x, f.y, f.w, f.h)
    } else if let Some(r) = &p.region {
        let v: Vec<i64> = r.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        if v.len() != 4 || v[2] <= 0 || v[3] <= 0 {
            return (StatusCode::BAD_REQUEST, "region은 x,y,w,h".to_string()).into_response();
        }
        dcli_raster::composite_region(doc, v[0] as i32, v[1] as i32, v[2] as u32, v[3] as u32)
    } else {
        dcli_raster::composite(doc)
    };
    let pixels = surface.to_srgb8_rgba();
    let mut buf = Vec::new();
    let mut enc = png::Encoder::new(&mut buf, surface.width(), surface.height());
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    if let Err(e) = enc.write_header().and_then(|mut w| w.write_image_data(&pixels)) {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("PNG 인코딩 실패: {e}"))
            .into_response();
    }
    ([(axum::http::header::CONTENT_TYPE, "image/png")], buf).into_response()
}

/// GET /doc/:id/state — 레이어 목록 + 문서 메타 + seq(읽기/디버그).
pub async fn state(State(app): Shared, Path(id): Path<String>) -> impl IntoResponse {
    let docs = app.docs.lock().unwrap();
    let Some(ds) = docs.get(&id) else {
        return doc_not_found(&id);
    };
    Json(json!({
        "seq": ds.seq,
        "doc": dto::doc_info_json(&ds.hist.doc),
        "layers": dto::layer_list_json(&ds.hist.doc)["layers"],
        "frames": dto::frames_json(&ds.hist.doc)["frames"],
        "can_undo": ds.hist.can_undo(),
        "can_redo": ds.hist.can_redo(),
    }))
    .into_response()
}

/// GET (WS) /doc/:id/live — 구독. hello(seq) 후 LiveMsg 스트림.
pub async fn live(
    State(app): Shared,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // 구독자 등록 + 현재 seq를 lock 안에서 스냅샷(원자적 hello).
    let sub = {
        let docs = app.docs.lock().unwrap();
        docs.get(&id).map(|ds| (ds.tx.subscribe(), ds.seq))
    };
    let Some((rx, seq)) = sub else {
        return (StatusCode::NOT_FOUND, format!("문서 '{id}' 없음")).into_response();
    };
    ws.on_upgrade(move |socket| live_socket(socket, rx, seq))
}

async fn live_socket(
    mut socket: WebSocket,
    mut rx: tokio::sync::broadcast::Receiver<LiveMsg>,
    seq: u64,
) {
    // hello: 클라가 자기 snapshot seq와 대조해 누락 검출.
    let hello = json!({ "type": "hello", "seq": seq }).to_string();
    if socket.send(Message::Text(hello)).await.is_err() {
        return;
    }
    loop {
        match rx.recv().await {
            Ok(msg) => {
                let txt = serde_json::to_string(&msg).unwrap();
                if socket.send(Message::Text(txt)).await.is_err() {
                    break; // 클라 끊김.
                }
            }
            // 느린 클라가 버퍼 초과로 밀림 → 끊어서 클라가 snapshot 재동기하게 함.
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                let _ = socket
                    .send(Message::Text(json!({ "type": "lagged" }).to_string()))
                    .await;
                break;
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}

fn doc_not_found(id: &str) -> axum::response::Response {
    (StatusCode::NOT_FOUND, format!("문서 '{id}' 없음")).into_response()
}
