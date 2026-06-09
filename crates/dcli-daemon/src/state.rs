//! 데몬 상태 — 메모리 다중 문서. 데몬이 **유일한 순서 결정자**다.
//!
//! 각 문서는 단조 증가 `seq`를 갖는다. 쓰기(apply/undo/redo)는 seq를 1 올리고
//! `LiveMsg`를 broadcast한다. 모든 클라(웹·CLI 읽기)는 동일 seq 스트림만 소비하므로
//! 자기 편집도 broadcast를 거쳐 돌아오고, echo 무시 로직 없이 순서가 일치한다.

use dcli_cli::dispatch::Action;
use dcli_model::History;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::broadcast;

/// WS로 broadcast되는 라이브 메시지(데몬이 정한 순서).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LiveMsg {
    /// Action batch 적용됨. 클라는 같은 actions를 wasm에 재적용.
    Ops { seq: u64, actions: Vec<Action> },
    /// undo 1단위. 클라는 editor.undo() 호출.
    Undo { seq: u64 },
    /// redo 1단위.
    Redo { seq: u64 },
}

/// 한 문서의 서버 사이드 상태.
pub struct DocState {
    pub hist: History,
    /// 단조 증가 편집 시퀀스. 0 = 생성 직후(편집 없음).
    pub seq: u64,
    /// 라이브 구독자에게 LiveMsg를 push하는 채널.
    pub tx: broadcast::Sender<LiveMsg>,
}

impl DocState {
    pub fn new(hist: History) -> Self {
        // 버퍼 256: 느린 클라가 잠깐 밀려도 hello→snapshot 재동기로 복구.
        let (tx, _rx) = broadcast::channel(256);
        DocState { hist, seq: 0, tx }
    }
}

/// 전역 앱 상태. doc_id → DocState.
///
/// 단일 사용자 로컬 프로토타입 → `std::sync::Mutex`로 충분(apply는 짧고 CPU 바운드).
/// async 핸들러 안에서 lock을 잡되 await를 넘기지 않으므로 블로킹 위험 없음.
pub struct AppState {
    pub docs: Mutex<HashMap<String, DocState>>,
}

impl AppState {
    pub fn new() -> Self {
        AppState { docs: Mutex::new(HashMap::new()) }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
