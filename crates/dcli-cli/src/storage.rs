//! 문서 영속화 — 문서 폴더(`.dxdoc/`)와 PNG export.
//!
//! 문서 = 폴더. 구조는 `doc.json`(픽셀 제외), 픽셀은 `pixels/<id>.bin` 사이드카로
//! 분리한다(document-model: 픽셀 JSON 인라인 금지). 코어는 bytes만 다루고 실제 파일
//! IO는 CLI가 담당한다(코어를 fs-무의존으로 유지 → WASM 호환).

use anyhow::{Context, Result};
use dcli_model::Document;
use dcli_tile::{PixelStore, Surface, SurfaceId};
use std::path::{Path, PathBuf};

const DOC_JSON: &str = "doc.json";
const PIXELS_DIR: &str = "pixels";

/// 문서 폴더 경로 래퍼.
pub struct DocPath(pub PathBuf);

impl DocPath {
    pub fn new(p: impl Into<PathBuf>) -> Self {
        DocPath(p.into())
    }

    fn json_path(&self) -> PathBuf {
        self.0.join(DOC_JSON)
    }

    fn pixels_dir(&self) -> PathBuf {
        self.0.join(PIXELS_DIR)
    }

    fn surface_path(&self, id: SurfaceId) -> PathBuf {
        self.pixels_dir().join(format!("{}.bin", id.0))
    }

    /// 폴더가 이미 문서로 보이는지(doc.json 존재).
    pub fn exists(&self) -> bool {
        self.json_path().is_file()
    }

    /// 문서를 폴더에 **원자적으로** 저장한다(구조 JSON + 픽셀 사이드카).
    ///
    /// 전략(검증 #1·#1b 반영): 새 내용을 임시 디렉토리에 전부 기록한 뒤 디렉토리 단위
    /// rename으로 교체한다. 중간 실패 시 기존 .dxdoc는 손상되지 않는다. 또한 현재
    /// 노드가 참조하는 SurfaceId의 사이드카만 기록하므로 stale `.bin`(orphan)이 남지 않는다.
    pub fn save(&self, doc: &Document) -> Result<()> {
        let target = &self.0;
        let parent = target.parent().filter(|p| !p.as_os_str().is_empty());
        // 임시 디렉토리(같은 부모에 둬야 rename이 같은 파일시스템 내에서 원자적).
        let tmp = match parent {
            Some(p) => p.join(format!(
                ".{}.tmp-{}",
                target.file_name().and_then(|s| s.to_str()).unwrap_or("doc"),
                std::process::id()
            )),
            None => PathBuf::from(format!(
                ".{}.tmp-{}",
                target.file_name().and_then(|s| s.to_str()).unwrap_or("doc"),
                std::process::id()
            )),
        };
        // 이전 잔여 tmp 정리.
        let _ = std::fs::remove_dir_all(&tmp);
        let tmp_pixels = tmp.join(PIXELS_DIR);
        std::fs::create_dir_all(&tmp_pixels)
            .with_context(|| format!("임시 폴더 생성 실패: {}", tmp_pixels.display()))?;

        // 구조 JSON.
        let json = doc.to_json().context("문서 JSON 직렬화 실패")?;
        std::fs::write(tmp.join(DOC_JSON), json).context("임시 doc.json 쓰기 실패")?;

        // 참조되는 표면만 기록(orphan 방지). id 오름차순 — 결정적.
        //
        // ★증분 저장★: 표면은 같은 id로 내용이 바뀌지 않으므로(재스타일은 새 sid 발급)
        // 기존 파일이 있으면 재직렬화 대신 hard-link로 재사용한다. 유일한 같은-sid 변형인
        // compact_text_surfaces는 크기가 줄어 파일 길이가 달라지므로 자동 재기록된다.
        // 이게 없으면 자동저장이 편집마다 문서 전체 픽셀(수십 MB)을 다시 쓰며 데몬
        // 뮤텍스를 수 초씩 점유한다(undo/적용/스냅샷이 줄을 서는 체감 멈춤의 원인).
        let referenced = doc.referenced_surfaces();
        let cur_pixels = target.join(PIXELS_DIR);
        for (id, surface) in doc.pixels().iter_sorted() {
            if !referenced.contains(&id) {
                continue; // orphan 표면은 디스크에 쓰지 않는다.
            }
            let dst = tmp_pixels.join(format!("{}.bin", id.0));
            let prev = cur_pixels.join(format!("{}.bin", id.0));
            let expected_len = 13u64 + (surface.width() as u64) * (surface.height() as u64) * 16;
            let reusable = std::fs::metadata(&prev)
                .map(|m| m.is_file() && m.len() == expected_len)
                .unwrap_or(false);
            if reusable && std::fs::hard_link(&prev, &dst).is_ok() {
                continue;
            }
            std::fs::write(&dst, surface.to_bytes())
                .with_context(|| format!("임시 픽셀 쓰기 실패: {id}"))?;
        }

        // 원자적 교체: 기존을 .bak로 옮기고 tmp를 본디렉토리로 rename.
        let mut bak = tmp.clone().into_os_string();
        bak.push(".bak");
        let bak = PathBuf::from(bak);
        let _ = std::fs::remove_dir_all(&bak);
        let had_existing = target.exists();
        if had_existing {
            std::fs::rename(target, &bak)
                .with_context(|| format!("기존 문서 백업 실패: {}", target.display()))?;
        }
        match std::fs::rename(&tmp, target) {
            Ok(()) => {
                let _ = std::fs::remove_dir_all(&bak);
                Ok(())
            }
            Err(e) => {
                // 교체 실패 → 백업 복구(가능하면).
                if had_existing {
                    let _ = std::fs::rename(&bak, target);
                }
                let _ = std::fs::remove_dir_all(&tmp);
                Err(anyhow::Error::new(e).context(format!("문서 교체 실패: {}", target.display())))
            }
        }
    }

    /// 문서를 폴더에서 로드한다(구조 + 픽셀 재주입).
    pub fn load(&self) -> Result<Document> {
        let json = std::fs::read_to_string(self.json_path())
            .with_context(|| format!("doc.json 읽기 실패: {}", self.json_path().display()))?;
        let mut doc = Document::from_json(&json).context("문서 JSON 파싱 실패")?;

        // 노드가 참조하는 모든 SurfaceId의 픽셀을 사이드카에서 로드.
        // ★그룹 자식 포함★ — 루트 order만 돌면 그룹 자식 표면이 누락되어 다음
        // save에서 사이드카가 유실된다(데이터 손실). referenced_surfaces는 전 노드 순회.
        let mut store = PixelStore::new();
        let ids: Vec<SurfaceId> = doc.referenced_surfaces().into_iter().collect();
        for id in ids {
            let path = self.surface_path(id);
            let bytes = std::fs::read(&path)
                .with_context(|| format!("픽셀 사이드카 없음: {}", path.display()))?;
            let surface = Surface::from_bytes(&bytes)
                .with_context(|| format!("픽셀 사이드카 손상: {}", path.display()))?;
            store.restore(id, surface);
        }
        doc.set_pixels(store);
        Ok(doc)
    }
}

/// 표면(또는 합성 결과)을 straight-alpha sRGB8 PNG로 export한다.
///
/// 결정성: png 라이브러리 기본 인코딩 사용. (flate 레벨 핀·canonical 순서 등 더 강한
/// 결정성 보장은 후속 — Phase 2에서는 동일 입력 → 동일 PNG 바이트만 보장.)
pub fn export_png(path: &Path, surface: &Surface) -> Result<()> {
    let file = std::fs::File::create(path)
        .with_context(|| format!("PNG 생성 실패: {}", path.display()))?;
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(file),
        surface.width(),
        surface.height(),
    );
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?
        .write_image_data(&surface.to_srgb8_rgba())?;
    Ok(())
}
