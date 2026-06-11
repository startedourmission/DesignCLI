//! Local PTY bridge for the editor's embedded agent terminal.
//!
//! This is intentionally narrow: the browser can only request known local tools
//! (`codex`, `claude`) or a login shell. It is not a general remote command API.

use crate::state::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::{header, StatusCode},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::ffi::CString;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::{Path as FsPath, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use tokio::sync::mpsc as tokio_mpsc;

#[derive(Deserialize)]
pub struct TerminalParams {
    cols: Option<u16>,
    rows: Option<u16>,
    doc: Option<String>,
}

#[derive(Deserialize)]
pub struct GuideParams {
    doc: Option<String>,
}

pub async fn terminal_guide(
    State(app): State<Arc<AppState>>,
    Query(params): Query<GuideParams>,
) -> impl IntoResponse {
    let path = params
        .doc
        .as_deref()
        .filter(|doc| valid_doc_id(doc))
        .map(|doc| app.projects_dir.join(format!("{doc}.dxdoc")))
        .filter(|path| path.is_dir())
        .and_then(|path| {
            if let Err(e) = seed_project_guide(&path) {
                tracing::warn!("프로젝트 터미널 가이드 생성 실패 {}: {}", path.display(), e);
            }
            let guide = path.join("AGENTS.md");
            guide.is_file().then_some(guide)
        })
        .unwrap_or_else(project_guide_template);
    match std::fs::read_to_string(&path) {
        Ok(text) => (
            [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
            text,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "terminal guide not found").into_response(),
    }
}

pub async fn open_terminal(
    State(app): State<Arc<AppState>>,
    Path(kind): Path<String>,
    Query(params): Query<TerminalParams>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    if terminal_argv(&kind).is_err() {
        return (
            StatusCode::BAD_REQUEST,
            "terminal kind must be codex, claude, or shell",
        )
            .into_response();
    }
    let cwd = match terminal_cwd(&app, params.doc.as_deref()) {
        Ok(path) => path,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };
    let env = terminal_env(params.doc.as_deref(), &cwd);
    let cols = params.cols.unwrap_or(100).clamp(20, 240);
    let rows = params.rows.unwrap_or(28).clamp(8, 80);
    ws.on_upgrade(move |socket| terminal_socket(socket, kind, cwd, env, cols, rows))
        .into_response()
}

async fn terminal_socket(
    socket: WebSocket,
    kind: String,
    cwd: PathBuf,
    env: Vec<(String, String)>,
    cols: u16,
    rows: u16,
) {
    let (mut sender, mut receiver) = socket.split();
    let cwd_display = cwd.display().to_string();
    let mut session = match spawn_pty(&kind, cwd, env, cols, rows) {
        Ok(s) => s,
        Err(e) => {
            let _ = sender
                .send(Message::Text(
                    json!({ "type": "error", "message": e.to_string() })
                        .to_string()
                        .into(),
                ))
                .await;
            return;
        }
    };

    let _ = sender
        .send(Message::Text(
            json!({ "type": "hello", "kind": kind, "cwd": cwd_display, "cols": cols, "rows": rows })
                .to_string()
                .into(),
        ))
        .await;

    let mut output = session.output.take().unwrap();
    loop {
        tokio::select! {
            next = receiver.next() => {
                let Some(Ok(msg)) = next else { break; };
                match msg {
                    Message::Binary(bytes) => {
                        if session.input.send(PtyInput::Data(bytes.to_vec())).is_err() {
                            break;
                        }
                    }
                    Message::Text(text) => {
                        if let Ok(msg) = serde_json::from_str::<ClientMsg>(&text) {
                            match msg {
                                ClientMsg::Resize { cols, rows } => {
                                    let cols = cols.clamp(20, 240);
                                    let rows = rows.clamp(8, 80);
                                    let _ = session.input.send(PtyInput::Resize { cols, rows });
                                }
                            }
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            next = output.recv() => {
                let Some(out) = next else { break; };
                match out {
                    PtyOutput::Data(bytes) => {
                        if sender.send(Message::Binary(bytes.into())).await.is_err() {
                            break;
                        }
                    }
                    PtyOutput::Exit(code) => {
                        let _ = sender
                            .send(Message::Text(
                                json!({ "type": "exit", "code": code }).to_string().into(),
                            ))
                            .await;
                        break;
                    }
                }
            }
        }
    }
    session.shutdown();
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMsg {
    Resize { cols: u16, rows: u16 },
}

enum PtyInput {
    Data(Vec<u8>),
    Resize { cols: u16, rows: u16 },
}

enum PtyOutput {
    Data(Vec<u8>),
    Exit(Option<i32>),
}

struct PtySession {
    input: mpsc::Sender<PtyInput>,
    output: Option<tokio_mpsc::UnboundedReceiver<PtyOutput>>,
    pid: libc::pid_t,
}

impl PtySession {
    fn shutdown(&self) {
        let _ = self.input.send(PtyInput::Data(vec![0x04]));
        unsafe {
            libc::kill(-self.pid, libc::SIGHUP);
            libc::kill(self.pid, libc::SIGHUP);
        }
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn terminal_argv(kind: &str) -> Result<Vec<CString>, anyhow::Error> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let command = match kind {
        "codex" => Some("codex"),
        "claude" => Some("claude"),
        "shell" => None,
        _ => anyhow::bail!("unknown terminal kind: {kind}"),
    };

    let mut argv = vec![CString::new(shell)?];
    if let Some(command) = command {
        argv.push(CString::new("-lc")?);
        argv.push(CString::new(format!(
            "export TERM=${{TERM:-xterm-256color}} COLORTERM=${{COLORTERM:-truecolor}}; exec {command}"
        ))?);
    } else {
        argv.push(CString::new("-l")?);
    }
    Ok(argv)
}

fn terminal_cwd(app: &AppState, doc: Option<&str>) -> Result<PathBuf, String> {
    let Some(doc) = doc.filter(|d| !d.trim().is_empty()) else {
        return std::env::current_dir().map_err(|e| e.to_string());
    };
    if !valid_doc_id(doc) {
        return Err("invalid doc id".to_string());
    }
    let path = app.projects_dir.join(format!("{doc}.dxdoc"));
    if path.is_dir() {
        if let Err(e) = seed_project_guide(&path) {
            tracing::warn!("프로젝트 터미널 가이드 생성 실패 {}: {}", path.display(), e);
        }
        Ok(path)
    } else {
        Err(format!("project '{doc}' not found"))
    }
}

fn valid_doc_id(id: &str) -> bool {
    !id.is_empty()
        && id != "."
        && id != ".."
        && !id.contains('/')
        && !id.contains('\\')
        && !id.ends_with(".dxdoc")
}

pub fn seed_project_guide(project_dir: &FsPath) -> std::io::Result<()> {
    let dst = project_dir.join("AGENTS.md");
    if dst.is_file() {
        return Ok(());
    }
    let text = std::fs::read_to_string(project_guide_template())?;
    std::fs::write(dst, text)
}

fn project_guide_template() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("templates")
        .join("project")
        .join("AGENTS.md")
}

fn terminal_env(doc: Option<&str>, cwd: &std::path::Path) -> Vec<(String, String)> {
    let mut env = vec![
        ("DX_PROJECT_DIR".to_string(), cwd.display().to_string()),
        ("DX_TERMINAL_CWD".to_string(), cwd.display().to_string()),
        ("DX_CLI_GUIDE".to_string(), "AGENTS.md".to_string()),
    ];
    // ★실시간 반영 보장★: dx CLI가 DX_SERVER/DX_DOC을 기본값으로 읽으므로(clap env),
    // 여기서 주입해 두면 터미널 안에서 `dx <verb>`만 쳐도 모든 편집이 데몬을 경유해
    // 열려 있는 에디터 화면에 즉시 broadcast된다(플래그 누락 = 디스크 새기 footgun 제거).
    let port = std::env::var("DX_PORT").unwrap_or_else(|_| "8137".into());
    env.push(("DX_SERVER".to_string(), format!("http://127.0.0.1:{port}")));
    if let Some(doc) = doc.filter(|d| !d.trim().is_empty()) {
        env.push(("DX_DOC_ID".to_string(), doc.to_string()));
        // cwd가 프로젝트 폴더 안이므로 상대경로는 깨진다 — 절대경로로.
        env.push(("DX_DOC".to_string(), cwd.display().to_string()));
    }
    // dx 바이너리를 PATH에 추가(데몬 실행 파일 옆 — target/debug 또는 설치 위치).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let old_path = std::env::var("PATH").unwrap_or_default();
            env.push(("PATH".to_string(), format!("{}:{}", dir.display(), old_path)));
        }
    }
    env
}

#[cfg(unix)]
fn spawn_pty(
    kind: &str,
    cwd: PathBuf,
    env: Vec<(String, String)>,
    cols: u16,
    rows: u16,
) -> Result<PtySession, anyhow::Error> {
    let argv = terminal_argv(kind)?;
    let argv_ptrs = argv
        .iter()
        .map(|s| s.as_ptr())
        .chain(std::iter::once(std::ptr::null()))
        .collect::<Vec<_>>();
    let cwd = CString::new(cwd.to_string_lossy().as_bytes())?;
    let env = env
        .into_iter()
        .map(|(k, v)| Ok((CString::new(k)?, CString::new(v)?)))
        .collect::<Result<Vec<_>, std::ffi::NulError>>()?;
    let fail_msg = format!("\r\nfailed to start {kind}\r\n").into_bytes();

    let mut master: libc::c_int = -1;
    let mut size = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let pid = unsafe {
        libc::forkpty(
            &mut master,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    };
    if pid < 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    if pid == 0 {
        unsafe {
            libc::chdir(cwd.as_ptr());
            libc::setenv(c"TERM".as_ptr(), c"xterm-256color".as_ptr(), 0);
            libc::setenv(c"COLORTERM".as_ptr(), c"truecolor".as_ptr(), 0);
            for (key, val) in &env {
                libc::setenv(key.as_ptr(), val.as_ptr(), 1);
            }
            libc::execvp(argv[0].as_ptr(), argv_ptrs.as_ptr());
            libc::write(
                libc::STDERR_FILENO,
                fail_msg.as_ptr().cast(),
                fail_msg.len(),
            );
            libc::_exit(127);
        }
    }

    let write_fd = unsafe { libc::dup(master) };
    if write_fd < 0 {
        unsafe {
            libc::close(master);
            libc::kill(pid, libc::SIGHUP);
        }
        return Err(std::io::Error::last_os_error().into());
    }

    let (input_tx, input_rx) = mpsc::channel::<PtyInput>();
    let (output_tx, output_rx) = tokio_mpsc::unbounded_channel::<PtyOutput>();

    std::thread::spawn(move || unsafe {
        let mut file = std::fs::File::from_raw_fd(write_fd);
        while let Ok(msg) = input_rx.recv() {
            match msg {
                PtyInput::Data(bytes) => {
                    if file.write_all(&bytes).is_err() {
                        break;
                    }
                    let _ = file.flush();
                }
                PtyInput::Resize { cols, rows } => {
                    let size = libc::winsize {
                        ws_row: rows,
                        ws_col: cols,
                        ws_xpixel: 0,
                        ws_ypixel: 0,
                    };
                    let _ = libc::ioctl(file.as_raw_fd(), libc::TIOCSWINSZ, &size);
                }
            }
        }
    });

    std::thread::spawn(move || unsafe {
        let mut file = std::fs::File::from_raw_fd(master);
        let mut buf = [0u8; 8192];
        loop {
            match file.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if output_tx.send(PtyOutput::Data(buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }

        let mut status: libc::c_int = 0;
        while libc::waitpid(pid, &mut status, 0) < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() != std::io::ErrorKind::Interrupted {
                break;
            }
        }
        let code = if libc::WIFEXITED(status) {
            Some(libc::WEXITSTATUS(status))
        } else if libc::WIFSIGNALED(status) {
            Some(128 + libc::WTERMSIG(status))
        } else {
            None
        };
        let _ = output_tx.send(PtyOutput::Exit(code));
    });

    Ok(PtySession {
        input: input_tx,
        output: Some(output_rx),
        pid,
    })
}

#[cfg(not(unix))]
fn spawn_pty(
    _kind: &str,
    _cwd: PathBuf,
    _env: Vec<(String, String)>,
    _cols: u16,
    _rows: u16,
) -> Result<PtySession, anyhow::Error> {
    anyhow::bail!("embedded terminal is only available on Unix platforms")
}
