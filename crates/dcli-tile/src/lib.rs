//! 64×64 타일 스토어.
//!
//! 픽셀은 절대 JSON에 인라인되지 않고(document-model 규칙) 여기 산다.
//! Phase 0에서는 단일 버퍼 raster surface로 최소 구현하되, 타일 격자 개념과
//! linear-premultiplied 저장 규약은 day-1부터 박는다. CoW/밉맵/dirty-rect는
//! 후속 Phase에서 이 표면 위에 얹는다.

#![forbid(unsafe_code)]

use dcli_color::LinearPremul;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 타일 한 변의 픽셀 수. document-model에서 확정.
pub const TILE_SIZE: u32 = 64;

/// 픽셀 표면 핸들. **노드/JSON은 이 id만 참조하고 픽셀은 인라인하지 않는다**
/// (document-model 규칙). 서버(스토어)가 발급하며 에이전트가 발명하지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SurfaceId(pub u64);

impl std::fmt::Display for SurfaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sfc{}", self.0)
    }
}

/// 픽셀 스토어. Surface를 id로 보관한다. JSON 직렬화 시 노드는 SurfaceId만 갖고,
/// 실제 픽셀은 이 스토어(추후 바이너리 사이드카)로 분리된다.
///
/// Phase 1은 인메모리 단순 맵. CoW/밉맵/타일 페이징은 후속 Phase에서 이 표면 위에 얹는다.
#[derive(Debug, Clone, Default)]
pub struct PixelStore {
    next: u64,
    map: HashMap<SurfaceId, Surface>,
}

impl PixelStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// 새 표면을 등록하고 발급된 id를 반환한다(서버측 id 발급).
    pub fn insert(&mut self, surface: Surface) -> SurfaceId {
        let id = SurfaceId(self.next);
        self.next += 1;
        self.map.insert(id, surface);
        id
    }

    pub fn get(&self, id: SurfaceId) -> Option<&Surface> {
        self.map.get(&id)
    }

    pub fn get_mut(&mut self, id: SurfaceId) -> Option<&mut Surface> {
        self.map.get_mut(&id)
    }

    /// 표면을 제거하고 반환한다(undo용 — 삭제 op의 역패치가 되돌릴 때 사용).
    pub fn remove(&mut self, id: SurfaceId) -> Option<Surface> {
        self.map.remove(&id)
    }

    /// 기존 id에 표면을 되돌려 넣는다(역패치 복원용, id 보존).
    pub fn restore(&mut self, id: SurfaceId, surface: Surface) {
        self.next = self.next.max(id.0 + 1);
        self.map.insert(id, surface);
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// linear-light, premultiplied alpha로 저장되는 픽셀 표면.
///
/// 합성기의 입출력 모두 이 표현을 쓴다. 색공간은 항상 linear-premul로 통일되며
/// (블렌드를 감마 공간에서 수행하더라도 저장은 linear), import/export 경계에서만
/// `dcli-color`의 contract로 sRGB와 변환한다.
#[derive(Debug, Clone)]
pub struct Surface {
    width: u32,
    height: u32,
    /// row-major, len == width*height.
    px: Vec<LinearPremul>,
}

impl Surface {
    /// 완전 투명으로 초기화된 표면.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            px: vec![LinearPremul::TRANSPARENT; (width * height) as usize],
        }
    }

    /// 단색(이미 linear-premul)으로 채운 표면.
    pub fn filled(width: u32, height: u32, color: LinearPremul) -> Self {
        Self {
            width,
            height,
            px: vec![color; (width * height) as usize],
        }
    }

    #[inline]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[inline]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// 이 표면을 덮는 64×64 타일 격자 크기 (가로, 세로 타일 수).
    pub fn tile_grid(&self) -> (u32, u32) {
        (
            self.width.div_ceil(TILE_SIZE),
            self.height.div_ceil(TILE_SIZE),
        )
    }

    #[inline]
    fn idx(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }

    #[inline]
    pub fn get(&self, x: u32, y: u32) -> LinearPremul {
        self.px[self.idx(x, y)]
    }

    #[inline]
    pub fn set(&mut self, x: u32, y: u32, c: LinearPremul) {
        let i = self.idx(x, y);
        self.px[i] = c;
    }

    /// 행 우선 픽셀 슬라이스(읽기).
    pub fn pixels(&self) -> &[LinearPremul] {
        &self.px
    }

    /// 행 우선 픽셀 슬라이스(쓰기).
    pub fn pixels_mut(&mut self) -> &mut [LinearPremul] {
        &mut self.px
    }

    /// straight-alpha sRGB8 RGBA 바이트로 export (export contract 적용).
    ///
    /// 결과는 PNG로 바로 쓸 수 있는 비-premultiplied 8bit RGBA.
    pub fn to_srgb8_rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.px.len() * 4);
        for p in &self.px {
            out.extend_from_slice(&p.to_srgb8_straight());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_grid_ceils() {
        let s = Surface::new(65, 128);
        assert_eq!(s.tile_grid(), (2, 2));
        let s = Surface::new(64, 64);
        assert_eq!(s.tile_grid(), (1, 1));
    }

    #[test]
    fn new_is_transparent() {
        let s = Surface::new(4, 4);
        assert_eq!(s.to_srgb8_rgba(), vec![0u8; 4 * 4 * 4]);
    }

    #[test]
    fn get_set_round_trip() {
        let mut s = Surface::new(2, 2);
        let c = LinearPremul::from_srgb8_straight(255, 0, 0, 255);
        s.set(1, 0, c);
        assert_eq!(s.get(1, 0), c);
    }
}
