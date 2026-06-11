//! `dx-daemon` — DesignCLI 라이브 동기화 데몬.
//!
//! 메모리에 다중 문서(History)를 들고, CLI/에이전트·웹이 같은 엔진(dispatch::apply_batch)으로
//! 편집하게 한다. 모든 쓰기는 seq를 올리고 WebSocket으로 broadcast → 웹 클라가 실시간 반영.
//! `dx-web/` 정적 파일도 함께 서빙하므로 `http://localhost:PORT/?doc=<id>` 한 곳에서 끝난다.
//!
//! 영속화: projects 디렉터리(DX_PROJECTS or ../../projects)에 <id>.dxdoc/ 폴더로 저장.
//! 자동저장 틱(500ms): dirty && 마지막 편집 후 1.5초 경과 시 백그라운드 저장.
//! Graceful shutdown: dirty 문서 전부 동기 저장.

mod routes;
mod state;
mod terminal;

use axum::{
    routing::{delete, get, post},
    Router,
};
use dcli_cli::storage::DocPath;
use state::AppState;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // workspace tracing-subscriber는 fmt만(env-filter feature 없음) → 단순 fmt 로거.
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let port: u16 = std::env::var("DX_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8137);
    let web_dir = web_dir();
    let projects_dir = projects_dir();

    // projects 디렉터리 없으면 생성.
    std::fs::create_dir_all(&projects_dir)?;
    tracing::info!("프로젝트 디렉터리: {}", projects_dir.display());

    // 시작 시 projects/*.dxdoc 폴더 목록 스캔(메모리 로드 안 함 — lazy).
    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        let names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir() && e.path().extension().map_or(false, |x| x == "dxdoc"))
            .filter_map(|e| {
                e.path()
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .collect();
        if !names.is_empty() {
            tracing::info!("디스크 프로젝트: {}", names.join(", "));
        }
    }

    let app_state = Arc::new(AppState::new(projects_dir));

    // 자동저장 틱 — 500ms 마다 dirty && 1.5초 경과 문서를 저장.
    let save_state = Arc::clone(&app_state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        loop {
            interval.tick().await;
            autosave_tick(&save_state);
        }
    });

    // API 라우트. 정적 서빙(ServeDir)을 fallback으로 둬서 /doc/* API가 우선.
    let api = Router::new()
        .route("/doc", get(routes::list_docs))
        .route("/doc/:id/create", post(routes::create_doc))
        .route("/doc/:id/apply", post(routes::apply))
        .route("/doc/:id/undo", post(routes::undo))
        .route("/doc/:id/redo", post(routes::redo))
        .route("/doc/:id/snapshot", get(routes::snapshot))
        .route("/doc/:id/snapshot.bin", get(routes::snapshot_bin))
        .route("/doc/:id/export.png", get(routes::export_png))
        .route("/doc/:id/thumb.png", get(routes::thumb_png))
        .route("/doc/:id/state", get(routes::state))
        .route("/doc/:id/live", get(routes::live))
        .route("/terminal/guide.md", get(terminal::terminal_guide))
        .route("/terminal/:kind", get(terminal::open_terminal))
        .route("/projects", get(routes::list_projects))
        .route("/projects/import-psd", post(routes::import_psd_project))
        .route("/projects/:name/rename", post(routes::rename_project))
        .route("/projects/:name", delete(routes::delete_project))
        .layer(axum::extract::DefaultBodyLimit::max(512 * 1024 * 1024))
        .with_state(Arc::clone(&app_state));

    let app = api
        .fallback_service(ServeDir::new(&web_dir))
        // 로컬 단일 사용자 — 개발 편의로 CORS 허용(다른 origin 정적 서버에서 붙는 경우).
        .layer(CorsLayer::permissive())
        // 정적 JS/wasm 캐시 금지 — 구버전 모듈 캐시로 "기능이 안 먹는" 사고 재발 방지.
        .layer(axum::middleware::map_response(no_store));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(
        "dx-daemon 기동: http://{addr}  (web: {})",
        web_dir.display()
    );
    tracing::info!("문서 생성 예: curl -X POST 'http://{addr}/doc/demo/create?w=800&h=600'");
    tracing::info!("브라우저:    http://{addr}/?doc=demo");
    tracing::info!("대시보드:    http://{addr}/");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Graceful shutdown: dirty 문서 전부 저장.
    flush_all(&app_state);
    tracing::info!("모든 dirty 문서 저장 완료, 종료.");
    Ok(())
}

/// 자동저장 틱 — dirty && 마지막 편집 후 1.5초 경과한 문서를 저장.
fn autosave_tick(state: &AppState) {
    use std::time::Duration;
    let debounce = Duration::from_millis(1500);
    // lock 안에서 저장 대상 선별(id + doc 복사) 후 unlock → 저장(I/O).
    // I/O를 lock 밖에서 하기 위해 Clone이 필요하지만 Document는 Clone 없음.
    // → 단순 전략: lock 안에서 DocPath::save를 직접 호출(짧은 I/O, 단일 사용자).
    let mut docs = state.docs.lock().unwrap();
    for (id, ds) in docs.iter_mut() {
        if !ds.dirty {
            continue;
        }
        let elapsed = ds.last_edit.map(|t| t.elapsed()).unwrap_or(debounce);
        if elapsed >= debounce {
            let path = DocPath::new(state.projects_dir.join(format!("{}.dxdoc", id)));
            match path.save(&ds.hist.doc) {
                Ok(()) => {
                    ds.dirty = false;
                    tracing::debug!("자동저장 완료: {}", id);
                }
                Err(e) => {
                    tracing::warn!("자동저장 실패 {}: {}", id, e);
                }
            }
        }
    }
}

/// Graceful shutdown 시 dirty 문서 전부 저장.
fn flush_all(state: &AppState) {
    let mut docs = state.docs.lock().unwrap();
    for (id, ds) in docs.iter_mut() {
        if !ds.dirty {
            continue;
        }
        let path = DocPath::new(state.projects_dir.join(format!("{}.dxdoc", id)));
        match path.save(&ds.hist.doc) {
            Ok(()) => {
                ds.dirty = false;
                tracing::info!("shutdown 저장 완료: {}", id);
            }
            Err(e) => {
                tracing::error!("shutdown 저장 실패 {}: {}", id, e);
            }
        }
    }
}

/// `dx-web/` 디렉터리 경로. DX_WEB_DIR 환경변수 우선, 없으면 워크스페이스 루트 기준.
fn web_dir() -> PathBuf {
    if let Ok(d) = std::env::var("DX_WEB_DIR") {
        return PathBuf::from(d);
    }
    // crates/dcli-daemon → ../../dx-web
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("..").join("..").join("dx-web")
}

/// projects 디렉터리. DX_PROJECTS 환경변수 우선, 없으면 저장소 루트/projects.
fn projects_dir() -> PathBuf {
    if let Ok(d) = std::env::var("DX_PROJECTS") {
        return PathBuf::from(d);
    }
    // crates/dcli-daemon → ../../projects
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("..").join("..").join("projects")
}

/// 모든 응답에 Cache-Control: no-store — 로컬 에디터라 캐시 이득이 없고 stale 모듈 사고만 만든다.
async fn no_store(mut res: axum::response::Response) -> axum::response::Response {
    res.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    res
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("종료 신호 수신, graceful shutdown");
}
