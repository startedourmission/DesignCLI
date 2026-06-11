//! 시스템/로컬 폰트 스캔 + 지연 등록 — 네이티브(CLI·데몬) 전용.
//!
//! 글꼴 디렉토리에서 TTF/OTF/TTC의 이름 테이블만 파싱해 (이름, 경로, face index)
//! 목록을 만든다. 실제 등록(`text::register_font`)은 이름이 쓰일 때 지연 수행.
//! wasm은 이 모듈이 없고, 데몬의 `/fonts/data`로 바이트를 받아 직접 등록한다.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// 스캔된 폰트 face 하나.
#[derive(Debug, Clone)]
pub struct SysFont {
    /// 표시 이름: "Family" 또는 "Family Subfamily"(Regular 외).
    pub name: String,
    pub path: PathBuf,
    pub index: u32,
}

/// 스캔 대상 디렉토리 — macOS 표준 + 레포 로컬 `fonts/`(무료 폰트 드롭 폴더).
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/System/Library/Fonts"),
        PathBuf::from("/System/Library/Fonts/Supplemental"),
        PathBuf::from("/Library/Fonts"),
        PathBuf::from("fonts"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(home).join("Library/Fonts"));
    }
    if let Ok(extra) = std::env::var("DX_FONT_DIRS") {
        dirs.extend(extra.split(':').map(PathBuf::from));
    }
    dirs
}

fn face_name(face: &ttf_parser::Face) -> Option<String> {
    // 영어(0x409) 이름 우선 — 일부 폰트는 첫 레코드가 CJK/로컬라이즈 이름이라
    // 목록·매칭이 들쭉날쭉해진다. 영어가 없으면 아무 유니코드 레코드나.
    let pick = |id: u16| {
        let by = |want_en: bool| {
            face.names().into_iter().find_map(|n| {
                if n.name_id != id || (want_en && n.language_id != 0x409) {
                    return None;
                }
                n.to_string().filter(|s| !s.trim().is_empty())
            })
        };
        by(true).or_else(|| by(false))
    };
    let family = pick(ttf_parser::name_id::TYPOGRAPHIC_FAMILY)
        .or_else(|| pick(ttf_parser::name_id::FAMILY))?;
    let sub = pick(ttf_parser::name_id::TYPOGRAPHIC_SUBFAMILY)
        .or_else(|| pick(ttf_parser::name_id::SUBFAMILY))
        .unwrap_or_default();
    Some(if sub.is_empty() || sub.eq_ignore_ascii_case("regular") {
        family
    } else {
        format!("{family} {sub}")
    })
}

fn scan_file(path: &Path, out: &mut Vec<SysFont>) {
    let Ok(bytes) = std::fs::read(path) else { return };
    let count = ttf_parser::fonts_in_collection(&bytes).unwrap_or(1);
    for index in 0..count {
        let Ok(face) = ttf_parser::Face::parse(&bytes, index) else {
            continue;
        };
        // 한글 미지원 글꼴은 제외 — 이 에디터의 1차 사용 언어가 한국어다.
        // (음절 2자 글리프 보유로 판정: 완성형 커버리지의 실용적 프록시.)
        let hangul = face.glyph_index('가').is_some() && face.glyph_index('한').is_some();
        if !hangul {
            continue;
        }
        if let Some(name) = face_name(&face) {
            out.push(SysFont {
                name,
                path: path.to_path_buf(),
                index,
            });
        }
    }
}

/// 전체 스캔(1회, 캐시). 이름 오름차순·중복 이름은 첫 항목 우선.
pub fn scan() -> &'static Vec<SysFont> {
    static CACHE: OnceLock<Vec<SysFont>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut found: Vec<SysFont> = Vec::new();
        for dir in font_dirs() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for entry in rd.flatten() {
                let path = entry.path();
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase());
                if matches!(ext.as_deref(), Some("ttf" | "otf" | "ttc" | "otc")) {
                    scan_file(&path, &mut found);
                }
            }
        }
        // macOS 히든 시스템 폰트(.접두)는 사용자 선택 대상이 아니다.
        found.retain(|f| !f.name.starts_with('.'));
        let mut seen = std::collections::HashSet::new();
        found.retain(|f| seen.insert(f.name.clone()));
        found.sort_by(|a, b| a.name.cmp(&b.name));
        found
    })
}

/// 이름으로 찾아 등록(지연). 이미 등록됐으면 즉시 true.
pub fn ensure(name: &str) -> bool {
    if crate::text::has_font(name) {
        return true;
    }
    let Some(f) = scan().iter().find(|f| f.name == name) else {
        return false;
    };
    let Ok(bytes) = std::fs::read(&f.path) else {
        return false;
    };
    crate::text::register_font(name, bytes, f.index).is_ok()
}

/// 액션/메타에 등장하는 폰트 이름을 일괄 ensure(미발견은 무시 — 번들 폴백).
pub fn ensure_all<'a>(names: impl IntoIterator<Item = &'a str>) {
    for n in names {
        let _ = ensure(n);
    }
}
