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
        /// 강제 NodeId. None=새로 발급(최초 적용), Some=그 id로 재생성(redo).
        /// ★redo가 같은 batch의 후속 op(이 노드를 참조)을 깨뜨리지 않으려면 redo 시
        /// 원래 발급 id를 보존해야 한다.★ 직렬화 제외(에이전트가 보내는 값 아님).
        #[serde(skip)]
        forced_id: Option<NodeId>,
    },
    /// 노드 삭제(역패치가 노드 전체 + 순서 위치를 복원). 그룹이면 자식까지 재귀 삭제·복원.
    DeleteLayer { id: NodeId },
    /// 노드를 새 순서 인덱스로 이동.
    MoveLayer { id: NodeId, to: usize },
    /// 노드 속성 일괄 변경(name/visible/opacity/blend).
    SetProps { id: NodeId, props: NodeProps },
    /// 페인트 노드의 표면 참조를 교체한다(재스타일/재래스터 — 노드 id·그룹 소속·z순서 보존).
    /// offset은 새 표면의 월드 원점(엔진 materialize origin). 그룹 자식에도 동작한다.
    ReplacePaintSurface {
        id: NodeId,
        surface: SurfaceId,
        offset: (i32, i32),
    },
    /// 최상위 노드들을 그룹으로 묶는다. ids는 루트 order에 있어야 하며 bottom-to-top
    /// 상대 순서를 유지한 채 그룹의 children이 된다. 그룹 노드는 멤버 중 가장 위
    /// 인덱스 위치에 들어간다.
    GroupLayers {
        ids: Vec<NodeId>,
        name: String,
        /// redo 시 같은 id 재사용(AddPaintLayer와 동일 규율). 직렬화 제외.
        #[serde(skip)]
        forced_id: Option<NodeId>,
    },
    /// 그룹 해제 — 자식들을 그룹이 있던 위치에 순서대로 펼친다.
    Ungroup { id: NodeId },
    /// Frame 목록 전체 교체(단순·원자적 — undo는 이전 목록 복원).
    SetFrames { frames: Vec<crate::Frame> },
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
    /// 표면 교체의 역 = 이전 표면 참조 + offset 복원(표면 객체는 스토어에 남아 있음).
    RestorePaintSurface {
        id: NodeId,
        surface: SurfaceId,
        offset: (i32, i32),
    },
    /// 그룹 삭제의 역 = 그룹 + 자식 노드 전체 + 표면들 복원(그룹 노드는 index 위치로).
    ReinsertDeletedGroup {
        group: Node,
        index: usize,
        children: Vec<Node>,
        surfaces: Vec<(SurfaceId, Surface)>,
    },
    /// 그룹 묶기의 역 = 그룹 제거 + 자식들을 원래 루트 인덱스로 복원.
    UngroupRestore {
        group_id: NodeId,
        /// (자식 id, 원래 루트 order 인덱스) — 인덱스 오름차순.
        restore: Vec<(NodeId, usize)>,
    },
    /// 그룹 해제의 역 = 그룹 노드 재구성(자식들을 다시 그룹 안으로).
    RegroupRestore { group: Node, index: usize },
    /// Frame 목록 복원.
    RestoreFrames { frames: Vec<crate::Frame> },
}

impl Op {
    /// op을 문서에 적용하고 역패치를 반환한다.
    pub fn apply(&self, doc: &mut Document) -> Result<Inverse, OpError> {
        match self {
            Op::AddPaintLayer {
                name,
                surface,
                index,
                forced_id,
            } => {
                if doc.pixels().get(*surface).is_none() {
                    return Err(OpError::SurfaceNotFound(*surface));
                }
                // forced_id가 있으면(redo) 그 id로, 없으면(최초) 새로 발급.
                // insert_node_at이 next_node를 max로 보정하므로 id 충돌 없음.
                let id = forced_id.unwrap_or_else(|| doc.alloc_node_id());
                let node = Node::paint(id, name.clone(), *surface);
                let idx = index.unwrap_or(doc.order().len());
                doc.insert_node_at(node, idx);
                Ok(Inverse::RemoveAdded { id })
            }
            Op::DeleteLayer { id } => {
                // ★검증 우선 규율★: 존재 확인 후에만 변형(부분 변형 금지).
                let is_group = matches!(
                    doc.get(*id).ok_or(OpError::NodeNotFound(*id))?.kind,
                    NodeKind::Group { .. }
                );
                if is_group {
                    // 그룹: 자식 노드 + 표면까지 재귀 회수(중첩 그룹 포함).
                    let (group, index) = doc.remove_node(*id).expect("존재 확인됨");
                    let mut children = Vec::new();
                    let mut surfaces = Vec::new();
                    let mut stack: Vec<NodeId> = match &group.kind {
                        NodeKind::Group { children } => children.clone(),
                        _ => unreachable!(),
                    };
                    while let Some(cid) = stack.pop() {
                        if let Some(child) = doc.take_node_entry(cid) {
                            if let NodeKind::Paint { surface } = child.kind {
                                if let Some(s) = doc.pixels_mut().remove(surface) {
                                    surfaces.push((surface, s));
                                }
                            }
                            if let NodeKind::Group { children } = &child.kind {
                                stack.extend(children.iter().copied());
                            }
                            children.push(child);
                        }
                    }
                    return Ok(Inverse::ReinsertDeletedGroup {
                        group,
                        index,
                        children,
                        surfaces,
                    });
                }
                let (node, index) = doc.remove_node(*id).ok_or(OpError::NodeNotFound(*id))?;
                // 페인트 노드면 표면도 함께 회수(undo 시 복원).
                let surface = match node.kind {
                    NodeKind::Paint { surface } => {
                        doc.pixels_mut().remove(surface).map(|s| (surface, s))
                    }
                    NodeKind::Group { .. } => None,
                };
                Ok(Inverse::ReinsertDeleted {
                    node,
                    index,
                    surface,
                })
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
                node.offset = props.offset;
                node.scale = props.scale;
                node.rotation = props.rotation;
                node.meta = props.meta.clone();
                Ok(Inverse::RestoreProps {
                    id: *id,
                    props: prev,
                })
            }
            Op::ReplacePaintSurface {
                id,
                surface,
                offset,
            } => {
                // ★검증 우선★: 새 표면이 스토어에 있고, 노드가 존재하는 페인트 노드여야 변형.
                if doc.pixels().get(*surface).is_none() {
                    return Err(OpError::SurfaceNotFound(*surface));
                }
                let node = doc.get_mut(*id).ok_or(OpError::NodeNotFound(*id))?;
                let NodeKind::Paint { surface: cur } = &mut node.kind else {
                    return Err(OpError::NodeNotFound(*id)); // 그룹에는 표면이 없다.
                };
                let old_sid = *cur;
                let old_off = node.offset;
                *cur = *surface;
                node.offset = *offset;
                // 옛 표면 객체는 스토어에 남긴다 — undo가 참조만 되돌리면 된다
                // (redo 역시 새 표면 참조 복귀; 고아 표면은 batch rollback 규율이 회수).
                Ok(Inverse::RestorePaintSurface {
                    id: *id,
                    surface: old_sid,
                    offset: old_off,
                })
            }
            Op::GroupLayers {
                ids,
                name,
                forced_id,
            } => {
                // ★검증 우선★: 전 멤버가 루트 order에 있고 중복이 없어야 변형 시작.
                if ids.is_empty() {
                    return Err(OpError::NodeNotFound(NodeId(u64::MAX)));
                }
                let mut indices = Vec::with_capacity(ids.len());
                for id in ids {
                    let idx = doc.order_index(*id).ok_or(OpError::NodeNotFound(*id))?;
                    indices.push((*id, idx));
                }
                // bottom-to-top 상대 순서 유지: 인덱스 오름차순 정렬.
                indices.sort_by_key(|&(_, i)| i);
                let top_index = indices.last().expect("비어있지 않음").1;
                // 자식들을 루트 order에서 제거(맵 유지). 위에서부터 제거해 인덱스 안정.
                for &(cid, _) in indices.iter().rev() {
                    doc.remove_from_order(cid);
                }
                // 그룹 노드 생성 — 멤버 중 최상단 위치(제거된 만큼 보정)에 삽입.
                let gid = forced_id.unwrap_or_else(|| doc.alloc_node_id());
                let children: Vec<NodeId> = indices.iter().map(|&(c, _)| c).collect();
                let mut group = Node::paint(gid, name.clone(), SurfaceId(0));
                group.kind = NodeKind::Group { children };
                let insert_at = top_index + 1 - indices.len();
                doc.insert_node_at(group, insert_at);
                Ok(Inverse::UngroupRestore {
                    group_id: gid,
                    restore: indices,
                })
            }
            Op::Ungroup { id } => {
                // ★검증 우선★: 그룹인지 확인 후 변형.
                let children = match &doc.get(*id).ok_or(OpError::NodeNotFound(*id))?.kind {
                    NodeKind::Group { children } => children.clone(),
                    _ => return Err(OpError::NodeNotFound(*id)),
                };
                let (group, index) = doc.remove_node(*id).expect("존재 확인됨");
                // 자식들을 그룹이 있던 위치에 bottom-to-top 순서대로 펼친다.
                for (i, cid) in children.iter().enumerate() {
                    doc.insert_order_at(*cid, index + i);
                }
                Ok(Inverse::RegroupRestore { group, index })
            }
            Op::SetFrames { frames } => {
                let prev = std::mem::replace(&mut doc.frames, frames.clone());
                Ok(Inverse::RestoreFrames { frames: prev })
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
            Inverse::ReinsertDeleted {
                node,
                index,
                surface,
            } => {
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
                node.offset = props.offset;
                node.scale = props.scale;
                node.rotation = props.rotation;
                node.meta = props.meta;
                Ok(())
            }
            Inverse::RestorePaintSurface {
                id,
                surface,
                offset,
            } => {
                let node = doc.get_mut(id).ok_or(OpError::NodeNotFound(id))?;
                let NodeKind::Paint { surface: cur } = &mut node.kind else {
                    return Err(OpError::NodeNotFound(id));
                };
                *cur = surface;
                node.offset = offset;
                Ok(())
            }
            Inverse::ReinsertDeletedGroup {
                group,
                index,
                children,
                surfaces,
            } => {
                for (sid, s) in surfaces {
                    doc.pixels_mut().restore(sid, s);
                }
                for child in children {
                    doc.put_node_entry(child);
                }
                doc.insert_node_at(group, index);
                Ok(())
            }
            Inverse::UngroupRestore { group_id, restore } => {
                // 그룹 노드 제거(자식 엔트리는 맵에 그대로) + 자식들을 원래 인덱스로.
                doc.remove_node(group_id)
                    .ok_or(OpError::NodeNotFound(group_id))?;
                for (cid, idx) in restore {
                    doc.insert_order_at(cid, idx);
                }
                Ok(())
            }
            Inverse::RegroupRestore { group, index } => {
                // 자식들을 루트 order에서 다시 빼고 그룹 노드 복원.
                if let NodeKind::Group { children } = &group.kind {
                    for cid in children.iter().rev() {
                        doc.remove_from_order(*cid);
                    }
                }
                doc.insert_node_at(group, index);
                Ok(())
            }
            Inverse::RestoreFrames { frames } => {
                doc.frames = frames;
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
    Op::AddPaintLayer {
        name: name.into(),
        surface: sid,
        index,
        forced_id: None,
    }
}

#[cfg(test)]
mod group_tests {
    use super::*;
    use crate::History;
    use dcli_color::BitDepth;

    fn doc3() -> History {
        // 레이어 3개(아래부터 a, b, c).
        let mut h = History::new(Document::new(8, 8, BitDepth::U8));
        for name in ["a", "b", "c"] {
            let op = add_paint_with_surface(&mut h.doc, name, Surface::new(8, 8), None);
            h.apply(op).unwrap();
        }
        h
    }

    #[test]
    fn group_then_ungroup_roundtrip() {
        let mut h = doc3();
        let (a, b) = (h.doc.order()[0], h.doc.order()[1]);
        h.apply(Op::GroupLayers {
            ids: vec![a, b],
            name: "g".into(),
            forced_id: None,
        })
        .unwrap();
        // 루트: [group, c] — 그룹은 멤버 최상단 위치(인덱스 1 → 제거 보정 후 0).
        assert_eq!(h.doc.order().len(), 2);
        let gid = h.doc.order()[0];
        match &h.doc.get(gid).unwrap().kind {
            NodeKind::Group { children } => assert_eq!(children, &vec![a, b]),
            _ => panic!("그룹이어야"),
        }
        // ungroup → 원래 평면 복원.
        h.apply(Op::Ungroup { id: gid }).unwrap();
        assert_eq!(h.doc.order(), &[a, b, h.doc.order()[2]]);
        assert!(h.doc.get(gid).is_none(), "그룹 노드 제거됨");
    }

    #[test]
    fn group_undo_restores_flat_order() {
        let mut h = doc3();
        let before: Vec<_> = h.doc.order().to_vec();
        let (a, b) = (before[0], before[1]);
        h.apply(Op::GroupLayers {
            ids: vec![a, b],
            name: "g".into(),
            forced_id: None,
        })
        .unwrap();
        assert!(h.undo().unwrap());
        assert_eq!(h.doc.order(), &before[..], "undo 후 평면 순서 복원");
        // redo는 같은 그룹 id 재사용(forced_id 규율).
        assert!(h.redo().unwrap());
        let gid1 = h.doc.order()[0];
        assert!(h.undo().unwrap());
        assert!(h.redo().unwrap());
        assert_eq!(h.doc.order()[0], gid1, "redo가 같은 그룹 id 재사용");
    }

    #[test]
    fn delete_group_restores_children_and_surfaces() {
        let mut h = doc3();
        let (a, b) = (h.doc.order()[0], h.doc.order()[1]);
        h.apply(Op::GroupLayers {
            ids: vec![a, b],
            name: "g".into(),
            forced_id: None,
        })
        .unwrap();
        let gid = h.doc.order()[0];
        let surfaces_before = h.doc.pixels().len();
        let nodes_before = h.doc.node_count();

        h.apply(Op::DeleteLayer { id: gid }).unwrap();
        assert_eq!(
            h.doc.pixels().len(),
            surfaces_before - 2,
            "자식 표면 2개 회수"
        );
        assert!(
            h.doc.get(a).is_none() && h.doc.get(b).is_none(),
            "자식 노드 제거"
        );

        h.undo().unwrap();
        assert_eq!(h.doc.pixels().len(), surfaces_before, "표면 복원");
        assert_eq!(h.doc.node_count(), nodes_before, "노드 복원");
        match &h.doc.get(gid).unwrap().kind {
            NodeKind::Group { children } => assert_eq!(children, &vec![a, b]),
            _ => panic!("그룹 복원"),
        }
    }

    #[test]
    fn set_frames_undo() {
        let mut h = History::new(Document::new(8, 8, BitDepth::U8));
        let f = crate::Frame {
            id: 0,
            name: "card".into(),
            x: -100,
            y: 50,
            w: 200,
            h: 100,
        };
        h.apply(Op::SetFrames {
            frames: vec![f.clone()],
        })
        .unwrap();
        assert_eq!(h.doc.frames.len(), 1);
        assert_eq!(h.doc.find_frame("card"), Some(&f));
        h.undo().unwrap();
        assert!(h.doc.frames.is_empty());
        h.redo().unwrap();
        assert_eq!(h.doc.frames.len(), 1);
    }
}
