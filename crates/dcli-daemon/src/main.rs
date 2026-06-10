//! `dx-daemon` — DesignCLI 라이브 동기화 데몬.
//!
//! 메모리에 다중 문서(History)를 들고, CLI/에이전트·웹이 같은 엔진(dispatch::apply_batch)으로
//! 편집하게 한다. 모든 쓰기는 seq를 올리고 WebSocket으로 broadcast → 웹 클라가 실시간 반영.
//! `dx-web/` 정적 파일도 함께 서빙하므로 `http://localhost:PORT/?doc=<id>` 한 곳에서 끝난다.

mod routes;
mod state;

use axum::{
    routing::{get, post},
    Router,
};
use state::AppState;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // workspace tracing-subscriber는 fmt만(env-filter feature 없음) → 단순 fmt 로거.
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();

    let port: u16 = std::env::var("DX_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(8137);
    let web_dir = web_dir();

    let app_state = Arc::new(AppState::new());

    // API 라우트. 정적 서빙(ServeDir)을 fallback으로 둬서 /doc/* API가 우선.
    let api = Router::new()
        .route("/doc", get(routes::list_docs))
        .route("/doc/:id/create", post(routes::create_doc))
        .route("/doc/:id/apply", post(routes::apply))
        .route("/doc/:id/undo", post(routes::undo))
        .route("/doc/:id/redo", post(routes::redo))
        .route("/doc/:id/snapshot", get(routes::snapshot))
        .route("/doc/:id/export.png", get(routes::export_png))
        .route("/doc/:id/state", get(routes::state))
        .route("/doc/:id/live", get(routes::live))
        .with_state(app_state);

    let app = api
        .fallback_service(ServeDir::new(&web_dir))
        // 로컬 단일 사용자 — 개발 편의로 CORS 허용(다른 origin 정적 서버에서 붙는 경우).
        .layer(CorsLayer::permissive())
        // 정적 JS/wasm 캐시 금지 — 구버전 모듈 캐시로 "기능이 안 먹는" 사고 재발 방지.
        .layer(axum::middleware::map_response(no_store));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("dx-daemon 기동: http://{addr}  (web: {})", web_dir.display());
    tracing::info!("문서 생성 예: curl -X POST 'http://{addr}/doc/demo/create?w=800&h=600'");
    tracing::info!("브라우저:    http://{addr}/?doc=demo");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
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
