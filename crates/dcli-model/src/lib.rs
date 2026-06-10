//! 문서 모델 (Phase 1: 균일 노드 트리 + op 스택 + event-sourced history).
//!
//! 사용자/에이전트가 보는 표면 = 직렬화 가능한 노드 트리(document-model). 모든 편집은
//! 직렬화 가능한 `Op`이고, 각 op는 적용 시 **역패치**(`Inverse`)를 생성한다 → undo는
//! 역패치 적용, redo는 op 재적용. 명령 로그가 append-only event-sourcing의 토대다.
//!
//! **픽셀은 JSON에 인라인되지 않는다** — 노드는 `SurfaceId`만 참조하고 실제 픽셀은
//! `PixelStore`(dcli-tile)에 분리 보관한다.
//!
//! 블렌드 모드의 *의미*(enum)는 여기서 단일 정의하고, 실제 픽셀 수학은
//! `dcli-raster`(CPU 정본)와 `dcli-gpu`(wgsl)가 동일하게 복제하되 parity 테스트로
//! 일치를 강제한다.

#![forbid(unsafe_code)]

use dcli_color::{BitDepth, BlendSpace};
use dcli_tile::{PixelStore, Surface, SurfaceId};
use serde::{Deserialize, Serialize};

mod history;
mod op;

pub mod fixtures;

pub use history::History;
pub use op::{Inverse, Op, OpError};

/// 고정 블렌드 모드 enum (document-model: 고정 enum, PSD 4자키와 양방향 매핑).
///
/// Phase 0~1에서는 separable 3종만. 나머지(Overlay/HardLight/non-separable 등)는
/// 후속 Phase에서 같은 enum에 추가하며, 추가 시마다 골든이미지로 고정한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
}

/// 노드 핸들. 서버가 발급하며 에이전트가 발명하지 않는다(cli-agent-interface).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(pub u64);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "n{}", self.0)
    }
}

/// 노드 종류 판별자 (document-model: 균일 노드 + kind). Phase 1은 paint/group만.
/// adjustment/text/shape/smartObject는 후속 Phase에서 같은 enum에 추가.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum NodeKind {
    /// 픽셀 레이어. 픽셀은 SurfaceId로만 참조(인라인 금지).
    Paint { surface: SurfaceId },
    /// 그룹. 자식 노드들을 묶는다(자식 순서는 Document.order로 관리).
    Group,
}

/// 균일 노드. 모든 노드가 같은 공통 필드 + kind를 갖는다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub visible: bool,
    /// [0,1] 불투명도.
    pub opacity: f32,
    pub blend: BlendMode,
    /// 캔버스 평행이동 (dx, dy) 정수 픽셀. 표면은 그대로 두고 합성 시 이만큼 시프트한다
    /// (Move 툴). 구버전 문서 호환 위해 default.
    #[serde(default)]
    pub offset: (i32, i32),
    /// 비파괴 스케일 (sx, sy). 표면 중심 기준, 합성 시 bilinear 리샘플. 음수 = 뒤집기.
    #[serde(default = "default_scale")]
    pub scale: (f32, f32),
    /// 비파괴 회전(도, 시계방향). 표면 중심 기준.
    #[serde(default)]
    pub rotation: f32,
    pub kind: NodeKind,
}

fn default_scale() -> (f32, f32) {
    (1.0, 1.0)
}

impl Node {
    pub fn paint(id: NodeId, name: impl Into<String>, surface: SurfaceId) -> Self {
        Self {
            id,
            name: name.into(),
            visible: true,
            opacity: 1.0,
            blend: BlendMode::Normal,
            offset: (0, 0),
            scale: (1.0, 1.0),
            rotation: 0.0,
            kind: NodeKind::Paint { surface },
        }
    }

    /// 트랜스폼이 identity(스케일 1, 회전 0)인지 — 합성기가 fast path 분기에 사용.
    pub fn is_identity_transform(&self) -> bool {
        self.scale == (1.0, 1.0) && self.rotation == 0.0
    }

    /// 이 노드가 참조하는 SurfaceId(있으면).
    pub fn surface_id(&self) -> Option<SurfaceId> {
        match self.kind {
            NodeKind::Paint { surface } => Some(surface),
            NodeKind::Group => None,
        }
    }
}

/// 노드의 직렬화 가능 속성(역패치가 통째로 저장/복원하는 단위).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeProps {
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub blend: BlendMode,
    /// 캔버스 평행이동 (dx, dy). SetProps/RestoreProps가 이걸 운반해 이동·undo가 자동 대칭.
    #[serde(default)]
    pub offset: (i32, i32),
    /// 비파괴 스케일 (sx, sy).
    #[serde(default = "default_scale")]
    pub scale: (f32, f32),
    /// 비파괴 회전(도).
    #[serde(default)]
    pub rotation: f32,
}

impl NodeProps {
    pub fn of(node: &Node) -> Self {
        Self {
            name: node.name.clone(),
            visible: node.visible,
            opacity: node.opacity,
            blend: node.blend,
            offset: node.offset,
            scale: node.scale,
            rotation: node.rotation,
        }
    }
}

/// 직렬화 가능한 문서 구조(픽셀 제외). JSON save/open이 이걸 직렬화한다.
///
/// Phase 1: 노드를 id→Node 맵 + bottom-to-top 순서 벡터로 표현(평면). 트리 중첩은
/// 후속 Phase에서 parent 포인터/자식 순서로 확장(현재는 모두 루트 자식).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub width: u32,
    pub height: u32,
    pub bit_depth: BitDepth,
    /// 합성 색공간(gamma-vs-linear-landmine: blendInLinear 비트 carry).
    pub blend_space: BlendSpace,
    /// id → 노드.
    nodes: std::collections::BTreeMap<NodeId, Node>,
    /// bottom-to-top 노드 순서(인덱스 0 = 맨 아래).
    order: Vec<NodeId>,
    /// 다음 발급할 NodeId.
    next_node: u64,
    /// 픽셀 스토어(직렬화 제외 — 바이너리 사이드카로 분리).
    #[serde(skip)]
    pixels: PixelStore,
}

impl Document {
    pub fn new(width: u32, height: u32, bit_depth: BitDepth) -> Self {
        Self {
            width,
            height,
            bit_depth,
            blend_space: BlendSpace::for_bit_depth(bit_depth),
            nodes: std::collections::BTreeMap::new(),
            order: Vec::new(),
            next_node: 0,
            pixels: PixelStore::new(),
        }
    }

    // ---- 픽셀 스토어 접근 ----

    pub fn pixels(&self) -> &PixelStore {
        &self.pixels
    }

    pub fn pixels_mut(&mut self) -> &mut PixelStore {
        &mut self.pixels
    }

    /// 픽셀 스토어를 통째로 교체한다(JSON 로드 후 바이너리 사이드카 주입용).
    pub fn set_pixels(&mut self, pixels: PixelStore) {
        self.pixels = pixels;
    }

    /// 픽셀 스토어를 꺼내고(소유권 이전) 문서에는 빈 스토어를 남긴다.
    pub fn take_pixels(&mut self) -> PixelStore {
        std::mem::take(&mut self.pixels)
    }

    /// 표면을 스토어에 등록하고 SurfaceId를 받는다(서버측 id 발급).
    pub fn add_surface(&mut self, surface: Surface) -> SurfaceId {
        self.pixels.insert(surface)
    }

    // ---- 노드 접근 ----

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(&id)
    }

    /// bottom-to-top 노드 id 순서.
    pub fn order(&self) -> &[NodeId] {
        &self.order
    }

    /// bottom-to-top 노드 참조 이터레이터(합성기가 사용).
    pub fn iter_bottom_to_top(&self) -> impl Iterator<Item = &Node> {
        self.order.iter().filter_map(move |id| self.nodes.get(id))
    }

    /// 현재 노드들이 참조하는 SurfaceId 집합(저장 시 이것만 디스크에 기록 → orphan 방지).
    pub fn referenced_surfaces(&self) -> std::collections::BTreeSet<SurfaceId> {
        self.nodes.values().filter_map(|n| n.surface_id()).collect()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 다음 NodeId를 발급한다.
    fn alloc_id(&mut self) -> NodeId {
        let id = NodeId(self.next_node);
        self.next_node += 1;
        id
    }

    // ---- 저수준 mutation (op 모듈이 사용; 직접 호출 지양) ----

    /// 노드를 주어진 순서 인덱스에 삽입한다.
    pub(crate) fn insert_node_at(&mut self, node: Node, index: usize) {
        let id = node.id;
        self.next_node = self.next_node.max(id.0 + 1);
        self.nodes.insert(id, node);
        let index = index.min(self.order.len());
        self.order.insert(index, id);
    }

    /// 노드를 제거하고 (제거된 노드, 순서 인덱스)를 반환한다.
    pub(crate) fn remove_node(&mut self, id: NodeId) -> Option<(Node, usize)> {
        let idx = self.order.iter().position(|&n| n == id)?;
        self.order.remove(idx);
        let node = self.nodes.remove(&id)?;
        Some((node, idx))
    }

    /// 순서 내 인덱스를 반환한다.
    pub(crate) fn order_index(&self, id: NodeId) -> Option<usize> {
        self.order.iter().position(|&n| n == id)
    }

    /// 노드를 새 순서 인덱스로 이동한다(반환: 이전 인덱스).
    pub(crate) fn move_node(&mut self, id: NodeId, to: usize) -> Option<usize> {
        let from = self.order_index(id)?;
        self.order.remove(from);
        let to = to.min(self.order.len());
        self.order.insert(to, id);
        Some(from)
    }

    /// JSON으로 직렬화(픽셀 제외).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// JSON에서 역직렬화(픽셀 스토어는 비어있게 복원 — 호출자가 픽셀을 채워야 함).
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

// op 모듈이 노드 생성을 위해 alloc_id를 쓸 수 있게 노출.
impl Document {
    pub(crate) fn alloc_node_id(&mut self) -> NodeId {
        self.alloc_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_blend_space_follows_depth() {
        let d8 = Document::new(8, 8, BitDepth::U8);
        assert_eq!(d8.blend_space, BlendSpace::Gamma);
        let d32 = Document::new(8, 8, BitDepth::F32);
        assert_eq!(d32.blend_space, BlendSpace::Linear);
    }

    #[test]
    fn json_excludes_pixels_but_keeps_surface_id() {
        let mut doc = Document::new(4, 4, BitDepth::U8);
        let sid = doc.add_surface(Surface::new(4, 4));
        let id = doc.alloc_node_id();
        doc.insert_node_at(Node::paint(id, "bg", sid), 0);

        let json = doc.to_json().unwrap();
        // SurfaceId는 JSON에 있지만 픽셀 데이터는 없어야 함.
        assert!(json.contains("surface"));
        assert!(!json.contains("LinearPremul"));

        let back = Document::from_json(&json).unwrap();
        assert_eq!(back.node_count(), 1);
        assert_eq!(back.get(id).unwrap().surface_id(), Some(sid));
        // 픽셀 스토어는 직렬화 제외 → 복원 시 비어있음.
        assert!(back.pixels().is_empty());
    }
}
