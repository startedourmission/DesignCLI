//! `dx-mcp` — DesignCLI MCP 서버(stdio).
//!
//! AI 에이전트가 직접 조작하는 인터페이스. CLI verb와 동일한 dispatch 엔진을 쓴다.
//! stdout은 JSON-RPC 전용(MUST) — 모든 로그는 stderr(tracing_subscriber writer=stderr).

use anyhow::Result;
use dcli_mcp::server::DesignServer;
use rmcp::transport::io::stdio;
use rmcp::ServiceExt;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // ★stdout 순수성★: 모든 진단 로그는 stderr로. stdout엔 JSON-RPC envelope만.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("dx-mcp 시작 (stdio)");
    let service = DesignServer::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
