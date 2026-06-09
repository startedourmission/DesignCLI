//! dcli-cli 라이브러리 — 문서 영속화(storage)와 JSON DTO(dto)를 CLI 바이너리·네이티브
//! 셸·MCP 서버가 공유한다.

pub mod dispatch;
pub mod dto;
// dxpkg는 코어(model/tile)만 의존 → wasm·native 양쪽에서 항상 컴파일(스냅샷 코덱 단일 진실원).
pub mod dxpkg;
// --server 모드 HTTP 클라(ureq). wasm 빌드에서 제외.
#[cfg(feature = "server")]
pub mod client;
// storage는 std::fs 의존 → wasm 빌드에서 제외(fs-sources off).
#[cfg(feature = "fs-sources")]
pub mod storage;
