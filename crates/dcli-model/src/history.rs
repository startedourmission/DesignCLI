//! Event-sourced 히스토리: append-only op 로그 + undo/redo.
//!
//! 적용된 모든 op을 순서대로 로그에 남기고, 각 op의 역패치를 undo 스택에 쌓는다.
//! undo = 역패치 적용 + redo 스택에 op 이동. redo = op 재적용.
//!
//! 명령이 이미 직렬화 데이터(`Op`)이므로 분기·감사·재현·스크립팅이 자연스럽다
//! (document-model). 새 op을 적용하면 redo 스택은 비워진다(표준 선형 히스토리).

use crate::op::{Inverse, Op, OpError};
use crate::Document;

/// 적용된 op과 그 역패치 한 쌍.
struct Entry {
    op: Op,
    inverse: Inverse,
}

/// 문서 + 편집 히스토리. 편집은 항상 이 핸들을 통해 적용해 로그/undo가 자동 유지된다.
pub struct History {
    pub doc: Document,
    /// append-only 적용 로그(undo 후에도 보존 — 감사/재현용).
    log: Vec<Op>,
    /// undo 가능한 항목(op + 역패치).
    done: Vec<Entry>,
    /// redo 가능한 op.
    undone: Vec<Op>,
}

impl History {
    pub fn new(doc: Document) -> Self {
        Self { doc, log: Vec::new(), done: Vec::new(), undone: Vec::new() }
    }

    /// op을 적용한다. 성공 시 역패치를 undo 스택에 쌓고 redo 스택을 비운다.
    pub fn apply(&mut self, op: Op) -> Result<(), OpError> {
        let inverse = op.apply(&mut self.doc)?;
        self.log.push(op.clone());
        self.done.push(Entry { op, inverse });
        self.undone.clear();
        Ok(())
    }

    /// 마지막 op을 되돌린다. 되돌릴 게 없으면 false.
    pub fn undo(&mut self) -> Result<bool, OpError> {
        let Some(entry) = self.done.pop() else {
            return Ok(false);
        };
        entry.inverse.apply(&mut self.doc)?;
        self.undone.push(entry.op);
        Ok(true)
    }

    /// 마지막으로 되돌린 op을 다시 적용한다. redo할 게 없으면 false.
    pub fn redo(&mut self) -> Result<bool, OpError> {
        let Some(op) = self.undone.pop() else {
            return Ok(false);
        };
        let inverse = op.apply(&mut self.doc)?;
        self.log.push(op.clone());
        self.done.push(Entry { op, inverse });
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
}
