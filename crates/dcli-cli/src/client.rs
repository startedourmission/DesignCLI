//! `--server` 모드 HTTP 클라 — CLI가 디스크 대신 데몬(`dx-daemon`)에 편집을 보낸다.
//!
//! 데몬이 유일한 순서 결정자이므로, CLI는 Action을 POST하고 데몬이 적용·broadcast한
//! 결과(BatchResult)를 받아 기존 Emitter로 출력한다. 디스크 경로와 **동일한 Action·결과
//! 타입**을 쓰므로 출력 표면이 한 코드경로로 일치한다.

use crate::dispatch::{Action, BatchResult};
use anyhow::{anyhow, Context, Result};
use serde_json::Value;

/// 데몬 접속 정보(서버 URL + 문서 id).
pub struct Server {
    base: String,
    doc_id: String,
}

impl Server {
    /// `base`는 "http://host:port"(끝 슬래시 무관), `doc_id`는 문서 식별자.
    pub fn new(base: &str, doc_id: &str) -> Self {
        Server { base: base.trim_end_matches('/').to_string(), doc_id: doc_id.to_string() }
    }

    fn url(&self, suffix: &str) -> String {
        format!("{}/doc/{}{}", self.base, self.doc_id, suffix)
    }

    /// Action 배열을 데몬에 적용한다. 응답의 BatchResult를 반환(디스크 apply_batch와 동형).
    pub fn apply(&self, actions: &[Action]) -> Result<BatchResult> {
        let body = serde_json::to_value(actions).context("Action 직렬화")?;
        let resp = ureq::post(&self.url("/apply"))
            .send_json(body)
            .map_err(map_ureq)?;
        let v: Value = resp.into_json().context("응답 JSON 파싱")?;
        // 데몬 응답: { seq, result: BatchResult }
        let result = v.get("result").ok_or_else(|| anyhow!("응답에 result 없음"))?;
        let br: BatchResult = serde_json::from_value(result.clone())
            .context("BatchResult 역직렬화")?;
        Ok(br)
    }

    /// undo 1단위. changed 여부 반환.
    pub fn undo(&self) -> Result<bool> {
        self.toggle("/undo")
    }

    /// redo 1단위.
    pub fn redo(&self) -> Result<bool> {
        self.toggle("/redo")
    }

    fn toggle(&self, suffix: &str) -> Result<bool> {
        let resp = ureq::post(&self.url(suffix)).call().map_err(map_ureq)?;
        let v: Value = resp.into_json().context("응답 JSON 파싱")?;
        Ok(v.get("changed").and_then(|b| b.as_bool()).unwrap_or(false))
    }

    /// GET /doc/:id/state — 레이어/메타 JSON(읽기 명령용).
    pub fn state(&self) -> Result<Value> {
        let resp = ureq::get(&self.url("/state")).call().map_err(map_ureq)?;
        resp.into_json().context("state JSON 파싱")
    }

    /// 문서를 생성한다(없으면). 이미 있으면 409를 무시하고 Ok.
    pub fn ensure_doc(&self, w: u32, h: u32, depth: &str) -> Result<()> {
        let url = format!("{}?w={}&h={}&depth={}", self.url("/create"), w, h, depth);
        match ureq::post(&url).call() {
            Ok(_) => Ok(()),
            // 409 = 이미 존재(정상). 그 외 status는 에러.
            Err(ureq::Error::Status(409, _)) => Ok(()),
            Err(e) => Err(map_ureq(e)),
        }
    }
}

/// ureq 에러를 사람이 읽을 anyhow로(상태코드 + 본문 메시지 포함).
fn map_ureq(e: ureq::Error) -> anyhow::Error {
    match e {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            anyhow!("데몬 {code}: {}", body.trim())
        }
        ureq::Error::Transport(t) => anyhow!("데몬 연결 실패: {t}"),
    }
}
