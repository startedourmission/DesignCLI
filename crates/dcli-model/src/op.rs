//! 편집 op과 역패치.
//!
//! 모든 편집은 직렬화 가능한 `Op`이다. 적용 시 `Inverse`(역패치)를 생성하며, undo는
//! 역패치 적용·redo는 op 재적용으로 이뤄진다(document-model: undo는 데이터).
//!
//! Phase 1은 구조/속성 op만: 레이어 추가/삭제/이동/속성변경. 픽셀 쓰기 op(fill/스트로크)는
//! 후속 Phase에서 추가하며 하이브리드 undo(타일 스냅샷)를 도입한다.
//!
//! ★롤백 비트동일성 불변식★ (batch 트랜잭션의 전제, History::rollback_to):
//! **모든 `Op::apply`는 검증을 모두 통과한 뒤에만 문서를 변형한다 — 부분 변형 후
//! 에러를 내면 안 된다.** 그래야 실패 시 이전 op들의 역패치만으로 문서를 정확히
//! 원복할 수 있다. forward에서 정규화(clamp 등)를 적용하면 inverse 복원도 대칭이어야
//! 한다(현재 SetProps는 prev를 이미-정규화된 현재값에서 캡처하므로 round-trip 일치).
//! 픽셀 쓰기 op 도입 시 이 대칭을 반드시 유지할 것.

use crate::{Document, Node, NodeId, NodeKind, NodeProps};
use dcli_tile::{Surface, SurfaceId};
use serde::{Deserialize, Serialize};

/// op 적용 실패 사유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpError {
    /// 참조한 노드가 없음.
    NodeNotFound(NodeId),
    /// 참조한 표면이 없음.
    SurfaceNotFound(SurfaceId),
}

impl std::fmt::Display for OpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpError::NodeNotFound(id) => write!(f, "노드 없음: {id}"),
            OpError::SurfaceNotFound(id) => write!(f, "표면 없음: {id}"),
        }
    }
}

impl std::error::Error for OpError {}

/// 편집 op. 직렬화 가능 → CLI/MCP batch가 op 배열을 트랜잭션 실행(cli-agent-interface).
///
/// 신규 노드 id는 에이전트가 발명하지 않는다 — `AddPaintLayer`는 서버가 id를 발급하고
/// 그 id를 역패치/결과로 돌려준다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// 이미 스토어에 등록된 표면을 참조하는 페인트 레이어를 순서 인덱스에 추가.
    AddPaintLayer {
        name: String,
        surface: SurfaceId,
        /// bottom-to-top 순서 인덱스(없으면 맨 위).
        index: Option<usize>,
    },
    /// 노드 삭제(역패치가 노드 전체 + 순서 위치를 복원).
    DeleteLayer { id: NodeId },
    /// 노드를 새 순서 인덱스로 이동.
    MoveLayer { id: NodeId, to: usize },
    /// 노드 속성 일괄 변경(name/visible/opacity/blend).
    SetProps { id: NodeId, props: NodeProps },
}

/// 역패치. undo 시 적용된다. op과 1:1 대응하지 않을 수 있다(예: 삭제의 역은 삽입).
///
/// **런타임 전용**(직렬화하지 않음): 회수한 `Surface` 픽셀을 들고 있어 JSON 직렬화
/// 대상이 아니다. 영속 히스토리는 직렬화 가능한 op 로그(`Op`)를 replay해 복원한다
/// (event-sourcing). 역패치는 in-memory undo 스택에서만 산다.
#[derive(Debug, Clone)]
pub enum Inverse {
    /// 추가의 역 = 삭제(발급된 id로).
    RemoveAdded { id: NodeId },
    /// 삭제의 역 = 노드 전체 + 순서 위치 + (있으면) 표면 복원.
    ReinsertDeleted {
        node: Node,
        index: usize,
        /// 삭제 시 스토어에서 함께 제거한 표면(있으면 복원).
        surface: Option<(SurfaceId, Surface)>,
    },
    /// 이동의 역 = 이전 인덱스로 되돌림.
    MoveBack { id: NodeId, to: usize },
    /// 속성 변경의 역 = 이전 속성 복원.
    RestoreProps { id: NodeId, props: NodeProps },
}

impl Op {
    /// op을 문서에 적용하고 역패치를 반환한다.
    pub fn apply(&self, doc: &mut Document) -> Result<Inverse, OpError> {
        match self {
            Op::AddPaintLayer { name, surface, index } => {
                if doc.pixels().get(*surface).is_none() {
                    return Err(OpError::SurfaceNotFound(*surface));
                }
                let id = doc.alloc_node_id();
                let node = Node::paint(id, name.clone(), *surface);
                let idx = index.unwrap_or(doc.order().len());
                doc.insert_node_at(node, idx);
                Ok(Inverse::RemoveAdded { id })
            }
            Op::DeleteLayer { id } => {
                let (node, index) = doc.remove_node(*id).ok_or(OpError::NodeNotFound(*id))?;
                // 페인트 노드면 표면도 함께 회수(undo 시 복원).
                let surface = match node.kind {
                    NodeKind::Paint { surface } => {
                        doc.pixels_mut().remove(surface).map(|s| (surface, s))
                    }
                    NodeKind::Group => None,
                };
                Ok(Inverse::ReinsertDeleted { node, index, surface })
            }
            Op::MoveLayer { id, to } => {
                let from = doc.move_node(*id, *to).ok_or(OpError::NodeNotFound(*id))?;
                Ok(Inverse::MoveBack { id: *id, to: from })
            }
            Op::SetProps { id, props } => {
                let node = doc.get_mut(*id).ok_or(OpError::NodeNotFound(*id))?;
                let prev = NodeProps::of(node);
                node.name = props.name.clone();
                node.visible = props.visible;
                node.opacity = props.opacity.clamp(0.0, 1.0);
                node.blend = props.blend;
                Ok(Inverse::RestoreProps { id: *id, props: prev })
            }
        }
    }
}

impl Inverse {
    /// 역패치를 적용한다(undo).
    pub fn apply(self, doc: &mut Document) -> Result<(), OpError> {
        match self {
            Inverse::RemoveAdded { id } => {
                let (node, _) = doc.remove_node(id).ok_or(OpError::NodeNotFound(id))?;
                // 추가했던 표면은 회수하지 않는다(원래 호출자 소유였을 수 있음).
                let _ = node;
                Ok(())
            }
            Inverse::ReinsertDeleted { node, index, surface } => {
                if let Some((sid, s)) = surface {
                    doc.pixels_mut().restore(sid, s);
                }
                doc.insert_node_at(node, index);
                Ok(())
            }
            Inverse::MoveBack { id, to } => {
                doc.move_node(id, to).ok_or(OpError::NodeNotFound(id))?;
                Ok(())
            }
            Inverse::RestoreProps { id, props } => {
                let node = doc.get_mut(id).ok_or(OpError::NodeNotFound(id))?;
                node.name = props.name;
                node.visible = props.visible;
                node.opacity = props.opacity;
                node.blend = props.blend;
                Ok(())
            }
        }
    }
}

/// 편의: 빈 표면을 스토어에 등록하고 그 위에 페인트 레이어를 추가하는 op을 만든다.
/// (테스트/스파이크용 — 실제 CLI는 표면 등록과 레이어 추가를 분리)
pub fn add_paint_with_surface(
    doc: &mut Document,
    name: impl Into<String>,
    surface: Surface,
    index: Option<usize>,
) -> Op {
    let sid = doc.add_surface(surface);
    Op::AddPaintLayer { name: name.into(), surface: sid, index }
}
