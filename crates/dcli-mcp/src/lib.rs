//! DesignCLI MCP 서버 라이브러리 — 세션 관리, 스냅샷, tool 서버.
//!
//! rmcp(tokio)를 이 crate에 가둔다. 코어 4종은 tokio/rmcp를 절대 링크하지 않는다.

pub mod server;
pub mod session;
pub mod snapshot;
