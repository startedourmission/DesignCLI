//! `--server` 모드 HTTP 클라 — CLI가 디스크 대신 데몬(`dx-daemon`)에 편집을 보낸다.
//!
//! 데몬이 유일한 순서 결정자이므로, CLI는 Action을 POST하고 데몬이 적용·broadcast한
//! 결과(BatchResult)를 받아 기존 Emitter로 출력한다. 디스크 경로와 **동일한 Action·결과
//! 타입**을 쓰므로 출력 표면이 한 코드경로로 일치한다.

use crate::dispatch::{Action, BatchResult};
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::io::Read;
use std::path::Path;

/// 데몬 접속 정보(서버 URL + 문서 id).
#[derive(Clone)]
pub struct Server {
    base: String,
    doc_id: String,
}

impl Server {
    /// `base`는 "http://host:port"(끝 슬래시 무관), `doc_id`는 문서 식별자.
    pub fn new(base: &str, doc_id: &str) -> Self {
        Server {
            base: base.trim_end_matches('/').to_string(),
            doc_id: doc_id.to_string(),
        }
    }

    /// 기본 로컬 데몬에서 이미 열린 `projects/<id>.dxdoc`를 찾으면 서버 모드로 전환한다.
    ///
    /// CLI가 디스크 문서를 직접 저장하면 이미 웹에서 열려 있는 메모리 문서와 갈라질 수 있다.
    /// 이 자동 감지는 그 경우에만 조용히 라이브 데몬을 진실원으로 사용한다. 데몬이 없거나,
    /// 프로젝트가 열려 있지 않거나, `projects/` 아래 문서가 아니면 기존 디스크 경로를 유지한다.
    pub fn auto_for_open_project(doc_path: &Path) -> Option<Self> {
        let parent = doc_path.parent()?;
        if parent.file_name().and_then(|s| s.to_str()) != Some("projects") {
            return None;
        }

        let id = doc_path.file_stem()?.to_str()?;
        let base = default_base();
        let url = format!("{base}/projects");
        // 자동 감지 프로브는 짧은 타임아웃 — 응답 없는 데몬(소켓만 열림)이 모든 CLI 명령을
        // 무기한 멈추게 두지 않는다. (ureq 기본값은 read timeout 없음.)
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_millis(300))
            .timeout(std::time::Duration::from_millis(1500))
            .build();
        let resp = agent.get(&url).call().ok()?;
        let projects: Value = resp.into_json().ok()?;
        let is_open = projects.as_array()?.iter().any(|p| {
            p.get("name").and_then(|v| v.as_str()) == Some(id)
                && p.get("open").and_then(|v| v.as_bool()) == Some(true)
        });
        if is_open {
            Some(Server::new(&base, id))
        } else {
            None
        }
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
        let result = v
            .get("result")
            .ok_or_else(|| anyhow!("응답에 result 없음"))?;
        let br: BatchResult =
            serde_json::from_value(result.clone()).context("BatchResult 역직렬화")?;
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

    /// GET /doc/:id/export.png — 데몬이 합성한 PNG를 받아 `out`에 저장한다.
    /// 성공 시 (width, height)를 반환(emit.exported 호환).
    pub fn export_png(&self, out: &Path) -> Result<(u32, u32)> {
        self.export_png_with(out, None, None)
    }

    /// frame(이름|id) 또는 region("x,y,w,h") 단위 export — 무한 작업영역의 캔버스 단위.
    pub fn export_png_with(
        &self,
        out: &Path,
        frame: Option<&str>,
        region: Option<&str>,
    ) -> Result<(u32, u32)> {
        let mut url = self.url("/export.png");
        if let Some(f) = frame {
            url = format!("{url}?frame={f}");
        } else if let Some(r) = region {
            url = format!("{url}?region={r}");
        }
        let resp = ureq::get(&url).call().map_err(map_ureq)?;
        let mut bytes = Vec::new();
        resp.into_reader()
            .read_to_end(&mut bytes)
            .context("PNG 응답 읽기")?;
        // 응답 헤더만 디코드해 크기 확인(전체 픽셀 디코드 없음) — 출력 메시지용.
        let info = png::Decoder::new(std::io::Cursor::new(&bytes))
            .read_info()
            .context("데몬 PNG 응답 파싱")?;
        let (w, h) = (info.info().width, info.info().height);
        std::fs::write(out, &bytes).with_context(|| format!("PNG 저장 실패: {}", out.display()))?;
        Ok((w, h))
    }

    /// 문서를 생성한다(없으면). 이미 있으면 409를 무시하고 Ok.
    /// 작업영역은 무한 — 크기 파라미터 없음(명목 크기는 데몬 기본값).
    pub fn ensure_doc(&self, depth: &str) -> Result<()> {
        let url = format!("{}?depth={}", self.url("/create"), depth);
        match ureq::post(&url).call() {
            Ok(_) => Ok(()),
            // 409 = 이미 존재(정상). 그 외 status는 에러.
            Err(ureq::Error::Status(409, _)) => Ok(()),
            Err(e) => Err(map_ureq(e)),
        }
    }
}

fn default_base() -> String {
    if let Ok(base) = std::env::var("DX_SERVER") {
        return base.trim_end_matches('/').to_string();
    }
    let port = std::env::var("DX_PORT").unwrap_or_else(|_| "8137".into());
    format!("http://localhost:{port}")
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
