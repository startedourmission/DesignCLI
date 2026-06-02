//! 문서 세션 — in-memory hold-open, 디스크가 정본.
//!
//! MCP 서버는 stateless per-tool load/save 대신 문서를 메모리에 들고 있는다(검증 #3):
//! History의 undo/redo 스택은 Document에 직렬화되지 않으므로, 매 tool마다 load하면
//! undo가 매번 비워져 history_undo/redo가 불가능하기 때문이다.
//!
//! DocId는 서버가 발급하는 핸들("doc-1")이다 — 에이전트가 경로/id를 발명하지 않는다.
//! 단일 편집기 락파일(.dxdoc/.lock)로 CLI/studio/MCP 동시 오픈을 배타화한다(검증 #3b).

use anyhow::{Context, Result};
use dcli_cli::storage::DocPath;
use dcli_model::{Document, History};
use std::collections::HashMap;
use std::path::PathBuf;

const LOCK_FILE: &str = ".lock";
const MAX_OPEN: usize = 16;

/// 서버 발급 문서 핸들.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocId(pub String);

/// 한 열린 문서: History(undo 포함) + 디스크 경로 + 락.
pub struct DocSession {
    pub history: History,
    pub path: DocPath,
    lock_path: PathBuf,
}

impl DocSession {
    /// autosave: 변경 후 원자적으로 디스크에 저장.
    pub fn save(&self) -> Result<()> {
        self.path.save(&self.history.doc)
    }

    /// 노드가 참조하나 PixelStore에 없는 표면(손상 가시화, 검증 #3b).
    pub fn missing_surfaces(&self) -> Vec<u64> {
        let doc = &self.history.doc;
        doc.referenced_surfaces()
            .into_iter()
            .filter(|sid| doc.pixels().get(*sid).is_none())
            .map(|sid| sid.0)
            .collect()
    }
}

impl Drop for DocSession {
    fn drop(&mut self) {
        // 세션 종료 시 락 해제.
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

/// 열린 문서들의 작업공간.
#[derive(Default)]
pub struct Workspace {
    sessions: HashMap<String, DocSession>,
    next_id: u64,
}

impl Workspace {
    pub fn new() -> Self {
        Self::default()
    }

    fn alloc_id(&mut self) -> DocId {
        let id = format!("doc-{}", self.next_id);
        self.next_id += 1;
        DocId(id)
    }

    fn lock_path_for(path: &std::path::Path) -> PathBuf {
        path.join(LOCK_FILE)
    }

    /// 이미 열린 같은 경로의 세션이 있으면 그 DocId 반환.
    fn find_open(&self, path: &std::path::Path) -> Option<DocId> {
        self.sessions
            .iter()
            .find(|(_, s)| s.path.0 == path)
            .map(|(id, _)| DocId(id.clone()))
    }

    /// 락 획득(다른 편집기가 열고 있으면 에러).
    fn acquire_lock(path: &std::path::Path) -> Result<PathBuf> {
        let lock = Self::lock_path_for(path);
        if lock.exists() {
            anyhow::bail!(
                "다른 편집기가 문서를 열고 있습니다(.lock 존재): {}",
                lock.display()
            );
        }
        if let Some(parent) = lock.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&lock, std::process::id().to_string())
            .with_context(|| format!("락 생성 실패: {}", lock.display()))?;
        Ok(lock)
    }

    /// 디스크의 .dxdoc 폴더를 세션으로 연다(이미 열렸으면 기존 DocId).
    pub fn open(&mut self, path: PathBuf) -> Result<DocId> {
        if let Some(id) = self.find_open(&path) {
            return Ok(id);
        }
        anyhow::ensure!(self.sessions.len() < MAX_OPEN, "열린 문서 한도 초과({MAX_OPEN})");
        let dp = DocPath::new(path.clone());
        anyhow::ensure!(dp.exists(), "문서가 없습니다: {}", path.display());
        let lock_path = Self::acquire_lock(&path)?;
        let doc = dp.load()?;
        let id = self.alloc_id();
        self.sessions.insert(
            id.0.clone(),
            DocSession { history: History::new(doc), path: dp, lock_path },
        );
        Ok(id)
    }

    /// 새 문서를 생성해 디스크에 저장하고 세션으로 연다.
    pub fn create(&mut self, path: PathBuf, doc: Document) -> Result<DocId> {
        anyhow::ensure!(self.sessions.len() < MAX_OPEN, "열린 문서 한도 초과({MAX_OPEN})");
        let dp = DocPath::new(path.clone());
        anyhow::ensure!(!dp.exists(), "이미 문서가 존재: {}", path.display());
        dp.save(&doc)?;
        let lock_path = Self::acquire_lock(&path)?;
        let id = self.alloc_id();
        self.sessions.insert(
            id.0.clone(),
            DocSession { history: History::new(doc), path: dp, lock_path },
        );
        Ok(id)
    }

    pub fn get(&self, id: &DocId) -> Option<&DocSession> {
        self.sessions.get(&id.0)
    }

    pub fn get_mut(&mut self, id: &DocId) -> Option<&mut DocSession> {
        self.sessions.get_mut(&id.0)
    }

    /// 세션을 닫는다(락 해제는 Drop에서).
    pub fn close(&mut self, id: &DocId) -> Option<DocSession> {
        self.sessions.remove(&id.0)
    }

    pub fn list(&self) -> Vec<(DocId, PathBuf, usize)> {
        self.sessions
            .iter()
            .map(|(id, s)| (DocId(id.clone()), s.path.0.clone(), s.history.doc.node_count()))
            .collect()
    }
}
