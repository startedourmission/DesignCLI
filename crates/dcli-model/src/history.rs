//! Event-sourced 히스토리: append-only op 로그 + undo/redo.
//!
//! 적용된 모든 op을 순서대로 로그에 남기고, 각 op의 역패치를 undo 스택에 쌓는다.
//! undo = 역패치 적용 + redo 스택에 op 이동. redo = op 재적용.
//!
//! 명령이 이미 직렬화 데이터(`Op`)이므로 분기·감사·재현·스크립팅이 자연스럽다
//! (document-model). 새 op을 적용하면 redo 스택은 비워진다(표준 선형 히스토리).

use crate::op::{Inverse, Op, OpError};
use crate::Document;

/// 적용된 op과 그 역패치 한 쌍. `group`이 같은 연속 entry는 한 batch로 묶여
/// undo/redo 시 하나의 논리 단위로 처리된다(None=단발 op, 자기 자신만).
struct Entry {
    op: Op,
    inverse: Inverse,
    group: Option<u64>,
}

/// redo 스택 항목. inverse는 op 재적용으로 새로 생기므로 op·group만 보존.
struct RedoEntry {
    op: Op,
    group: Option<u64>,
}

/// 문서 + 편집 히스토리. 편집은 항상 이 핸들을 통해 적용해 로그/undo가 자동 유지된다.
///
/// **트랜잭션(batch):** `savepoint()`로 현재 위치를 기억하고 op들을 stage한 뒤,
/// 전부 성공하면 `commit_batch(sp)`로 batch 전체를 undo 1단위로 묶고, 하나라도
/// 실패하면 `rollback_to(sp)`로 LIFO 역적용해 **문서를 비트 단위로 원복**한다.
/// (단, batch가 PixelStore에 등록한 표면의 회수는 호출자 책임 — RemoveAdded가
/// 표면을 회수하지 않으므로. dispatch가 owned_surfaces를 추적해 reclaim한다.)
pub struct History {
    pub doc: Document,
    /// append-only 적용 로그(undo 후에도 보존 — 감사/재현용).
    log: Vec<Op>,
    /// undo 가능한 항목(op + 역패치).
    done: Vec<Entry>,
    /// redo 가능한 항목(op + group, inverse는 재적용 시 재생성).
    undone: Vec<RedoEntry>,
    /// 다음 batch group id.
    next_group: u64,
}

impl History {
    pub fn new(doc: Document) -> Self {
        Self { doc, log: Vec::new(), done: Vec::new(), undone: Vec::new(), next_group: 0 }
    }

    /// op을 적용한다(단발). 성공 시 역패치를 undo 스택에 쌓고 redo 스택을 비운다.
    pub fn apply(&mut self, op: Op) -> Result<(), OpError> {
        let inverse = op.apply(&mut self.doc)?;
        self.log.push(op.clone());
        self.done.push(Entry { op, inverse, group: None });
        self.undone.clear();
        Ok(())
    }

    // ---- 트랜잭션 API ----

    /// 현재 undo 스택 길이를 savepoint로 반환한다.
    pub fn savepoint(&self) -> usize {
        self.done.len()
    }

    /// op을 적용해 undo 스택에 stage한다(batch 내부용 — 아직 group 미지정).
    /// log/redo는 commit 시까지 건드리지 않는다.
    pub fn stage(&mut self, op: Op) -> Result<(), OpError> {
        let inverse = op.apply(&mut self.doc)?;
        self.done.push(Entry { op, inverse, group: None });
        Ok(())
    }

    /// savepoint 이후 stage된 모든 op을 역적용해 문서를 원복한다(batch 실패 시).
    /// **표면 회수는 하지 않는다** — 호출자가 owned_surfaces로 별도 reclaim.
    pub fn rollback_to(&mut self, sp: usize) -> Result<(), OpError> {
        while self.done.len() > sp {
            let entry = self.done.pop().expect("savepoint 일관성");
            entry.inverse.apply(&mut self.doc)?;
        }
        Ok(())
    }

    /// savepoint 이후 stage된 op들을 하나의 batch group으로 commit한다.
    /// log에 반영하고 redo 스택을 비운다. batch 전체가 undo 1단위가 된다.
    pub fn commit_batch(&mut self, sp: usize) {
        if self.done.len() <= sp {
            return; // 빈 batch.
        }
        let group = self.next_group;
        self.next_group += 1;
        for entry in &mut self.done[sp..] {
            entry.group = Some(group);
            self.log.push(entry.op.clone());
        }
        self.undone.clear();
    }

    // ---- undo/redo (batch group 인지) ----

    /// 마지막 논리 단위(단발 op 또는 batch group 전체)를 되돌린다. 없으면 false.
    ///
    /// group이 None인 단발 entry는 1개만, Some(g)인 batch entry는 같은 group이
    /// 연속되는 동안 전부 역적용한다(LIFO).
    pub fn undo(&mut self) -> Result<bool, OpError> {
        let Some(group) = self.done.last().map(|e| e.group) else {
            return Ok(false);
        };
        loop {
            let entry = self.done.pop().expect("최소 1개 보장됨");
            // ★redo 안정성★: AddPaintLayer는 redo 시 같은 NodeId로 재생성해야 같은
            // batch의 후속 op(이 노드 참조)이 안 깨진다. inverse가 아는 발급 id를
            // op의 forced_id로 박아 undone에 넣는다.
            let mut op = entry.op;
            if let (Op::AddPaintLayer { forced_id, .. }, Inverse::RemoveAdded { id }) =
                (&mut op, &entry.inverse)
            {
                *forced_id = Some(*id);
            }
            entry.inverse.apply(&mut self.doc)?;
            self.undone.push(RedoEntry { op, group: entry.group });
            // 단발이면 1개로 종료. batch면 다음 entry가 같은 group일 때만 계속.
            if group.is_none() || self.done.last().map(|e| e.group) != Some(group) {
                break;
            }
        }
        Ok(true)
    }

    /// 마지막으로 되돌린 논리 단위를 다시 적용한다. 없으면 false.
    pub fn redo(&mut self) -> Result<bool, OpError> {
        let Some(group) = self.undone.last().map(|e| e.group) else {
            return Ok(false);
        };
        loop {
            let entry = self.undone.pop().expect("최소 1개 보장됨");
            let inverse = entry.op.apply(&mut self.doc)?;
            self.log.push(entry.op.clone());
            self.done.push(Entry { op: entry.op, inverse, group });
            if group.is_none() || self.undone.last().map(|e| e.group) != Some(group) {
                break;
            }
        }
        Ok(true)
    }

    /// 지금까지 적용된(undo 포함) 전체 op 로그(append-only, 감사용).
    pub fn log(&self) -> &[Op] {
        &self.log
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::add_paint_with_surface;
    use crate::{BlendMode, NodeProps};
    use dcli_color::BitDepth;
    use dcli_tile::Surface;

    fn doc() -> Document {
        Document::new(8, 8, BitDepth::U8)
    }

    #[test]
    fn add_then_undo_redo() {
        let mut h = History::new(doc());
        let op = add_paint_with_surface(&mut h.doc, "bg", Surface::new(8, 8), None);
        h.apply(op).unwrap();
        assert_eq!(h.doc.node_count(), 1);

        assert!(h.undo().unwrap());
        assert_eq!(h.doc.node_count(), 0);

        assert!(h.redo().unwrap());
        assert_eq!(h.doc.node_count(), 1);
    }

    #[test]
    fn delete_undo_restores_surface() {
        let mut h = History::new(doc());
        let op = add_paint_with_surface(&mut h.doc, "bg", Surface::new(8, 8), None);
        h.apply(op).unwrap();
        let id = h.doc.order()[0];
        let sid_before = h.doc.get(id).unwrap().surface_id().unwrap();
        let surfaces_before = h.doc.pixels().len();

        h.apply(Op::DeleteLayer { id }).unwrap();
        assert_eq!(h.doc.node_count(), 0);
        // 삭제 시 표면도 회수.
        assert_eq!(h.doc.pixels().len(), surfaces_before - 1);

        // undo → 노드와 표면 모두 복원(같은 SurfaceId 보존).
        h.undo().unwrap();
        assert_eq!(h.doc.node_count(), 1);
        assert_eq!(h.doc.pixels().len(), surfaces_before);
        assert_eq!(h.doc.get(id).unwrap().surface_id().unwrap(), sid_before);
        assert!(h.doc.pixels().get(sid_before).is_some());
    }

    #[test]
    fn set_props_undo() {
        let mut h = History::new(doc());
        let op = add_paint_with_surface(&mut h.doc, "bg", Surface::new(8, 8), None);
        h.apply(op).unwrap();
        let id = h.doc.order()[0];

        h.apply(Op::SetProps {
            id,
            props: NodeProps {
                name: "renamed".into(),
                visible: false,
                opacity: 0.5,
                blend: BlendMode::Multiply,
                offset: (0, 0),
                scale: (1.0, 1.0),
                rotation: 0.0,
            },
        })
        .unwrap();
        assert_eq!(h.doc.get(id).unwrap().opacity, 0.5);
        assert_eq!(h.doc.get(id).unwrap().blend, BlendMode::Multiply);

        h.undo().unwrap();
        let n = h.doc.get(id).unwrap();
        assert_eq!(n.name, "bg");
        assert_eq!(n.opacity, 1.0);
        assert_eq!(n.blend, BlendMode::Normal);
        assert!(n.visible);
    }

    #[test]
    fn move_undo() {
        let mut h = History::new(doc());
        for i in 0..3 {
            let op = add_paint_with_surface(&mut h.doc, format!("l{i}"), Surface::new(8, 8), None);
            h.apply(op).unwrap();
        }
        let order0 = h.doc.order().to_vec();
        let top = order0[2];

        // 맨 위(idx 2) → 맨 아래(idx 0)로 이동.
        h.apply(Op::MoveLayer { id: top, to: 0 }).unwrap();
        assert_eq!(h.doc.order()[0], top);

        h.undo().unwrap();
        assert_eq!(h.doc.order(), &order0[..]);
    }

    #[test]
    fn apply_clears_redo() {
        let mut h = History::new(doc());
        let op = add_paint_with_surface(&mut h.doc, "a", Surface::new(8, 8), None);
        h.apply(op).unwrap();
        h.undo().unwrap();
        assert!(h.can_redo());

        // 새 op 적용 → redo 무효화.
        let op = add_paint_with_surface(&mut h.doc, "b", Surface::new(8, 8), None);
        h.apply(op).unwrap();
        assert!(!h.can_redo());
    }

    #[test]
    fn log_is_append_only_through_undo() {
        let mut h = History::new(doc());
        let op = add_paint_with_surface(&mut h.doc, "a", Surface::new(8, 8), None);
        h.apply(op).unwrap();
        h.undo().unwrap();
        // undo해도 로그는 보존(감사용).
        assert_eq!(h.log().len(), 1);
        h.redo().unwrap();
        assert_eq!(h.log().len(), 2);
    }

    // ---- 트랜잭션 / round-trip 불변식 ----

    /// (노드 구조, 픽셀 바이트) 다이제스트 타입.
    type Digest = (Vec<(u64, String, f32, bool)>, Vec<(u64, Vec<u8>)>);

    /// 문서의 구조+픽셀 다이제스트(round-trip 비트동일 검증용).
    fn digest(doc: &Document) -> Digest {
        let structure: Vec<_> = doc
            .order()
            .iter()
            .map(|id| {
                let n = doc.get(*id).unwrap();
                (id.0, n.name.clone(), n.opacity, n.visible)
            })
            .collect();
        let pixels: Vec<_> = doc.pixels().iter_sorted().map(|(id, s)| (id.0, s.to_bytes())).collect();
        (structure, pixels)
    }

    #[test]
    fn rollback_restores_structure_caller_reclaims_surface() {
        // 계약: rollback_to는 구조(노드 트리)를 비트 단위 원복하되, batch가
        // PixelStore에 등록한 표면은 회수하지 않는다(RemoveAdded가 표면 미회수).
        // 표면 회수는 호출자(dispatch)가 owned_surfaces로 별도 수행 — 여기서 그 계약을 검증.
        let mut h = History::new(doc());
        for i in 0..2 {
            let op = add_paint_with_surface(&mut h.doc, format!("base{i}"), Surface::new(8, 8), None);
            h.apply(op).unwrap();
        }
        let before = digest(&h.doc);
        let sp = h.savepoint();

        // batch stage: 레이어 추가(표면 등록됨) + 속성 변경 + 이동.
        let op = add_paint_with_surface(&mut h.doc, "tmp", Surface::new(8, 8), None);
        let Op::AddPaintLayer { surface: tmp_sid, .. } = &op else { unreachable!() };
        let tmp_sid = *tmp_sid;
        h.stage(op).unwrap();
        let new_id = *h.doc.order().last().unwrap();
        h.stage(Op::SetProps {
            id: new_id,
            props: NodeProps { name: "x".into(), visible: false, opacity: 0.3, blend: BlendMode::Screen, offset: (0, 0), scale: (1.0, 1.0), rotation: 0.0 },
        })
        .unwrap();
        h.stage(Op::MoveLayer { id: new_id, to: 0 }).unwrap();
        assert_ne!(digest(&h.doc).0, before.0, "stage 후엔 구조가 달라야");

        // rollback → 구조는 원복.
        h.rollback_to(sp).unwrap();
        assert_eq!(digest(&h.doc).0, before.0, "rollback 후 구조 비트동일이어야");
        // 그러나 등록했던 표면은 orphan으로 남아있다(History는 회수 안 함).
        assert!(h.doc.pixels().get(tmp_sid).is_some(), "History rollback은 표면 미회수가 계약");

        // 호출자가 owned surface를 reclaim → 픽셀까지 완전 원복.
        h.doc.pixels_mut().remove(tmp_sid);
        assert_eq!(digest(&h.doc), before, "reclaim 후 구조+픽셀 모두 비트동일");
    }

    #[test]
    fn commit_batch_is_single_undo_unit() {
        let mut h = History::new(doc());
        let before_count = h.doc.node_count();
        let sp = h.savepoint();

        // batch: 레이어 3개 추가.
        for i in 0..3 {
            let op = add_paint_with_surface(&mut h.doc, format!("l{i}"), Surface::new(8, 8), None);
            h.stage(op).unwrap();
        }
        h.commit_batch(sp);
        assert_eq!(h.doc.node_count(), before_count + 3);

        // undo 한 번 → batch 전체(3개)가 한꺼번에 사라짐.
        assert!(h.undo().unwrap());
        assert_eq!(h.doc.node_count(), before_count);

        // redo 한 번 → batch 전체 복원.
        assert!(h.redo().unwrap());
        assert_eq!(h.doc.node_count(), before_count + 3);
    }

    #[test]
    fn single_op_undo_is_one_step_even_after_batch() {
        let mut h = History::new(doc());
        // batch 1개(2 레이어).
        let sp = h.savepoint();
        for i in 0..2 {
            let op = add_paint_with_surface(&mut h.doc, format!("b{i}"), Surface::new(8, 8), None);
            h.stage(op).unwrap();
        }
        h.commit_batch(sp);
        // 단발 1개.
        let op = add_paint_with_surface(&mut h.doc, "solo", Surface::new(8, 8), None);
        h.apply(op).unwrap();
        assert_eq!(h.doc.node_count(), 3);

        // undo 1회 → 단발만(solo) 사라짐, batch는 유지.
        h.undo().unwrap();
        assert_eq!(h.doc.node_count(), 2);
        // undo 2회 → batch 전체 사라짐.
        h.undo().unwrap();
        assert_eq!(h.doc.node_count(), 0);
    }
}
