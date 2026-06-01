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

    /// 문서를 폴더에 저장한다(구조 JSON + 픽셀 사이드카).
    pub fn save(&self, doc: &Document) -> Result<()> {
        std::fs::create_dir_all(self.pixels_dir())
            .with_context(|| format!("픽셀 폴더 생성 실패: {}", self.pixels_dir().display()))?;
        // 구조 JSON.
        let json = doc.to_json().context("문서 JSON 직렬화 실패")?;
        std::fs::write(self.json_path(), json)
            .with_context(|| format!("doc.json 쓰기 실패: {}", self.json_path().display()))?;
        // 픽셀 사이드카(id 오름차순 — 결정적).
        for (id, surface) in doc.pixels().iter_sorted() {
            std::fs::write(self.surface_path(id), surface.to_bytes())
                .with_context(|| format!("픽셀 쓰기 실패: {id}"))?;
        }
        Ok(())
    }

    /// 문서를 폴더에서 로드한다(구조 + 픽셀 재주입).
    pub fn load(&self) -> Result<Document> {
        let json = std::fs::read_to_string(self.json_path())
            .with_context(|| format!("doc.json 읽기 실패: {}", self.json_path().display()))?;
        let mut doc = Document::from_json(&json).context("문서 JSON 파싱 실패")?;

        // 노드가 참조하는 모든 SurfaceId의 픽셀을 사이드카에서 로드.
        let mut store = PixelStore::new();
        let ids: Vec<SurfaceId> = doc
            .order()
            .iter()
            .filter_map(|nid| doc.get(*nid).and_then(|n| n.surface_id()))
            .collect();
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
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), surface.width(), surface.height());
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&surface.to_srgb8_rgba())?;
    Ok(())
}
