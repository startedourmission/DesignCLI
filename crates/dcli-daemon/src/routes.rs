//! HTTP/WS 라우트 — 데몬의 공개 API.
//!
//! 쓰기는 전부 `dispatch::apply_batch`(또는 History undo/redo) 단일 경로를 거치고
//! seq를 올린 뒤 broadcast한다.
//!
//! Lazy-open: 메모리에 없는 id를 만나면 디스크에서 로드해 DocState 생성 후 진행.
//! 없으면 404.

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
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

type Shared = State<Arc<AppState>>;

// ─────────────────────── lazy-open 헬퍼 ───────────────────────

/// 메모리에서 문서를 찾거나, 디스크에서 lazy load한다.
/// None이면 디스크에도 없는 것(→ 404).
fn ensure_doc<'a>(
    app: &'a AppState,
    id: &str,
) -> Option<()> {
    let mut docs = app.docs.lock().unwrap();
    if docs.contains_key(id) {
        return Some(());
    }
    // 디스크에 있으면 로드.
    let path = app.doc_path(id);
    if !path.exists() {
        return None;
    }
    match path.load() {
        Ok(doc) => {
            tracing::info!("lazy load: {}", id);
            docs.insert(id.to_string(), DocState::new(History::new(doc)));
            Some(())
        }
        Err(e) => {
            tracing::error!("lazy load 실패 {}: {}", id, e);
            None
        }
    }
}

// ─────────────────────── POST /doc/:id/create ───────────────────────

/// POST /doc/:id/create?w&h&depth — 새 문서 등록 + 디스크 즉시 저장.
#[derive(Deserialize)]
pub struct CreateParams {
    #[serde(default = "default_w")]
    w: u32,
    #[serde(default = "default_h")]
    h: u32,
    #[serde(default = "default_depth")]
    depth: String,
}
fn default_w() -> u32 { 512 }
fn default_h() -> u32 { 512 }
fn default_depth() -> String { "u8".into() }

fn parse_depth(s: &str) -> Result<BitDepth, String> {
    match s {
        "u8"  => Ok(BitDepth::U8),
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
    {
        let docs = app.docs.lock().unwrap();
        if docs.contains_key(&id) {
            return (StatusCode::CONFLICT, format!("문서 '{id}' 이미 존재")).into_response();
        }
    }
    // 디스크에도 이미 있으면 충돌.
    let path = app.doc_path(&id);
    if path.exists() {
        return (StatusCode::CONFLICT, format!("프로젝트 '{id}' 이미 존재(디스크)")).into_response();
    }
    let doc = Document::new(p.w, p.h, depth);
    // 디스크에 즉시 저장(폴더 생성).
    if let Err(e) = path.save(&doc) {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("디스크 저장 실패: {e}")).into_response();
    }
    let mut docs = app.docs.lock().unwrap();
    docs.insert(id.clone(), DocState::new(History::new(doc)));
    (StatusCode::CREATED, Json(json!({ "id": id, "w": p.w, "h": p.h, "depth": p.depth, "seq": 0 })))
        .into_response()
}

// ─────────────────────── GET /doc ───────────────────────

/// GET /doc — 열린(메모리 로드된) 문서 id 목록.
pub async fn list_docs(State(app): Shared) -> impl IntoResponse {
    let docs = app.docs.lock().unwrap();
    let ids: Vec<&String> = docs.keys().collect();
    Json(json!({ "docs": ids })).into_response()
}

// ─────────────────────── GET /projects ───────────────────────

/// GET /projects 응답 엔트리.
#[derive(Serialize)]
struct ProjectEntry {
    name: String,
    /// 메모리에 로드되어 있는지.
    open: bool,
    /// 파일 mtime ISO 문자열(doc.json 기준). 실패 시 null.
    modified: Option<String>,
    /// 문서 폭(열려 있으면 메모리, 아니면 doc.json 파싱).
    w: Option<u32>,
    /// 문서 높이.
    h: Option<u32>,
}

pub async fn list_projects(State(app): Shared) -> impl IntoResponse {
    use std::time::SystemTime;

    let projects_dir = app.projects_dir.clone();
    let docs_lock = app.docs.lock().unwrap();

    // projects/*.dxdoc 폴더 열거.
    let entries = match std::fs::read_dir(&projects_dir) {
        Ok(e) => e,
        Err(_) => return Json(json!([])).into_response(),
    };

    let mut result: Vec<ProjectEntry> = Vec::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() { continue; }
        let ext = path.extension().and_then(|s| s.to_str());
        if ext != Some("dxdoc") { continue; }
        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        let open = docs_lock.contains_key(&name);

        // mtime: doc.json 기준.
        let doc_json = path.join("doc.json");
        let modified = std::fs::metadata(&doc_json)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                // SystemTime → ISO 8601 문자열(chrono 없이 직접 계산).
                let duration = t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
                let secs = duration.as_secs();
                // 간단한 ISO 문자열(초 단위, UTC).
                unix_to_iso(secs)
            });

        // 크기: 열려 있으면 메모리, 아니면 doc.json 경량 파싱.
        let (w, h) = if open {
            let ds = docs_lock.get(&name).unwrap();
            (Some(ds.hist.doc.width), Some(ds.hist.doc.height))
        } else if doc_json.is_file() {
            // doc.json에서 width/height만 파싱(전체 Document 로드 안 함).
            parse_doc_size(&doc_json)
        } else {
            (None, None)
        };

        result.push(ProjectEntry { name, open, modified, w, h });
    }

    // 이름 오름차순 정렬.
    result.sort_by(|a, b| a.name.cmp(&b.name));
    Json(result).into_response()
}

/// Unix 초 → "YYYY-MM-DDTHH:MM:SSZ" (UTC, chrono 불필요).
fn unix_to_iso(secs: u64) -> String {
    // 율리우스 달력 계산(간단 버전).
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    // 1970-01-01 기준 날짜 계산.
    let z = days + 719468;
    let era = z / 146097;
    let doe = z % 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, s)
}

/// doc.json에서 width/height만 경량 파싱(전체 Document 로드 없이).
fn parse_doc_size(path: &std::path::Path) -> (Option<u32>, Option<u32>) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return (None, None),
    };
    // serde_json Value로 최소한만 파싱.
    let v: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return (None, None),
    };
    let w = v["width"].as_u64().map(|n| n as u32);
    let h = v["height"].as_u64().map(|n| n as u32);
    (w, h)
}

// ─────────────────────── DELETE /projects/:name ───────────────────────

pub async fn delete_project(
    State(app): Shared,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let path = app.projects_dir.join(format!("{}.dxdoc", name));
    if !path.exists() {
        // 메모리에도 없으면 404.
        let exists_mem = app.docs.lock().unwrap().contains_key(&name);
        if !exists_mem {
            return (StatusCode::NOT_FOUND, format!("프로젝트 '{name}' 없음")).into_response();
        }
    }
    // 메모리에서 제거.
    app.docs.lock().unwrap().remove(&name);
    // 디스크 폴더 삭제.
    if path.exists() {
        if let Err(e) = std::fs::remove_dir_all(&path) {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("폴더 삭제 실패: {e}")).into_response();
        }
    }
    tracing::info!("프로젝트 삭제: {}", name);
    (StatusCode::OK, Json(json!({ "deleted": name }))).into_response()
}

// ─────────────────────── 편집 라우트 (lazy-open) ───────────────────────

/// POST /doc/:id/apply — Action 배열 적용(CLI·웹 공통 쓰기 경로).
pub async fn apply(
    State(app): Shared,
    Path(id): Path<String>,
    Json(actions): Json<Vec<Action>>,
) -> impl IntoResponse {
    if ensure_doc(&app, &id).is_none() {
        return doc_not_found(&id);
    }
    let mut docs = app.docs.lock().unwrap();
    let ds = docs.get_mut(&id).unwrap();
    let res = dispatch::apply_batch(&mut ds.hist, &actions, false);
    if res.ok {
        ds.seq += 1;
        ds.mark_dirty();
        let _ = ds.tx.send(LiveMsg::Ops { seq: ds.seq, actions });
    }
    (StatusCode::OK, Json(json!({ "seq": ds.seq, "result": serde_json::to_value(&res).unwrap() })))
        .into_response()
}

/// POST /doc/:id/undo
pub async fn undo(State(app): Shared, Path(id): Path<String>) -> impl IntoResponse {
    if ensure_doc(&app, &id).is_none() {
        return doc_not_found(&id);
    }
    let mut docs = app.docs.lock().unwrap();
    let ds = docs.get_mut(&id).unwrap();
    match ds.hist.undo() {
        Ok(true) => {
            ds.seq += 1;
            ds.mark_dirty();
            let _ = ds.tx.send(LiveMsg::Undo { seq: ds.seq });
            Json(json!({ "ok": true, "changed": true, "seq": ds.seq })).into_response()
        }
        Ok(false) => Json(json!({ "ok": true, "changed": false, "seq": ds.seq })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// POST /doc/:id/redo
pub async fn redo(State(app): Shared, Path(id): Path<String>) -> impl IntoResponse {
    if ensure_doc(&app, &id).is_none() {
        return doc_not_found(&id);
    }
    let mut docs = app.docs.lock().unwrap();
    let ds = docs.get_mut(&id).unwrap();
    match ds.hist.redo() {
        Ok(true) => {
            ds.seq += 1;
            ds.mark_dirty();
            let _ = ds.tx.send(LiveMsg::Redo { seq: ds.seq });
            Json(json!({ "ok": true, "changed": true, "seq": ds.seq })).into_response()
        }
        Ok(false) => Json(json!({ "ok": true, "changed": false, "seq": ds.seq })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ─────────────────────── 읽기 라우트 ───────────────────────

/// GET /doc/:id/snapshot — {seq, dxpkg_base64}. 웹 초기화용.
pub async fn snapshot(State(app): Shared, Path(id): Path<String>) -> impl IntoResponse {
    if ensure_doc(&app, &id).is_none() {
        return doc_not_found(&id);
    }
    let docs = app.docs.lock().unwrap();
    let ds = docs.get(&id).unwrap();
    let bytes = dxpkg::encode(&ds.hist.doc);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Json(json!({ "seq": ds.seq, "dxpkg_base64": b64 })).into_response()
}

/// GET /doc/:id/export.png — 합성 결과를 8bit RGBA PNG로 응답(디스크 export와 동일 인코딩).
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
    if ensure_doc(&app, &id).is_none() {
        return doc_not_found(&id);
    }
    let docs = app.docs.lock().unwrap();
    let ds = docs.get(&id).unwrap();
    let doc = &ds.hist.doc;
    let surface = if let Some(key) = &p.frame {
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
    png_response(surface)
}

/// GET /doc/:id/thumb.png — 최대 256px nearest 다운스케일 PNG(대시보드 카드용).
pub async fn thumb_png(
    State(app): Shared,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if ensure_doc(&app, &id).is_none() {
        return doc_not_found(&id);
    }
    let docs = app.docs.lock().unwrap();
    let ds = docs.get(&id).unwrap();
    let surface = dcli_raster::composite(&ds.hist.doc);
    let tw = surface.width();
    let th = surface.height();
    const MAX: u32 = 256;
    if tw <= MAX && th <= MAX {
        // 이미 충분히 작으면 그대로.
        return png_response(surface);
    }
    // Nearest 다운스케일.
    let scale = (MAX as f32 / tw.max(th) as f32).min(1.0);
    let nw = ((tw as f32 * scale).round() as u32).max(1);
    let nh = ((th as f32 * scale).round() as u32).max(1);
    let src_pixels = surface.to_srgb8_rgba();
    let mut dst = vec![0u8; (nw * nh * 4) as usize];
    for dy in 0..nh {
        for dx in 0..nw {
            let sx = ((dx as f32 / nw as f32) * tw as f32) as u32;
            let sy = ((dy as f32 / nh as f32) * th as f32) as u32;
            let si = ((sy * tw + sx) * 4) as usize;
            let di = ((dy * nw + dx) * 4) as usize;
            dst[di..di + 4].copy_from_slice(&src_pixels[si..si + 4]);
        }
    }
    let mut buf = Vec::new();
    let mut enc = png::Encoder::new(&mut buf, nw, nh);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    if let Err(e) = enc.write_header().and_then(|mut w| w.write_image_data(&dst)) {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("PNG 인코딩 실패: {e}")).into_response();
    }
    ([(axum::http::header::CONTENT_TYPE, "image/png")], buf).into_response()
}

/// GET /doc/:id/state — 레이어 목록 + 문서 메타 + seq(읽기/디버그).
pub async fn state(State(app): Shared, Path(id): Path<String>) -> impl IntoResponse {
    if ensure_doc(&app, &id).is_none() {
        return doc_not_found(&id);
    }
    let docs = app.docs.lock().unwrap();
    let ds = docs.get(&id).unwrap();
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

// ─────────────────────── WS /doc/:id/live ───────────────────────

/// GET (WS) /doc/:id/live — 구독. hello(seq) 후 LiveMsg 스트림.
pub async fn live(
    State(app): Shared,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    if ensure_doc(&app, &id).is_none() {
        return (StatusCode::NOT_FOUND, format!("문서 '{id}' 없음")).into_response();
    }
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
    if socket.send(Message::Text(hello.into())).await.is_err() {
        return;
    }
    loop {
        match rx.recv().await {
            Ok(msg) => {
                let txt = serde_json::to_string(&msg).unwrap();
                if socket.send(Message::Text(txt.into())).await.is_err() {
                    break; // 클라 끊김.
                }
            }
            // 느린 클라가 버퍼 초과로 밀림 → 끊어서 클라가 snapshot 재동기하게 함.
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                let _ = socket
                    .send(Message::Text(json!({ "type": "lagged" }).to_string().into()))
                    .await;
                break;
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}

// ─────────────────────── 공용 헬퍼 ───────────────────────

fn doc_not_found(id: &str) -> axum::response::Response {
    (StatusCode::NOT_FOUND, format!("문서 '{id}' 없음")).into_response()
}

fn png_response(surface: dcli_tile::Surface) -> axum::response::Response {
    let pixels = surface.to_srgb8_rgba();
    let mut buf = Vec::new();
    let mut enc = png::Encoder::new(&mut buf, surface.width(), surface.height());
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    if let Err(e) = enc.write_header().and_then(|mut w| w.write_image_data(&pixels)) {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("PNG 인코딩 실패: {e}")).into_response();
    }
    ([(axum::http::header::CONTENT_TYPE, "image/png")], buf).into_response()
}
