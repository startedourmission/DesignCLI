// AppController — Editor(wasm) 핸들이 유일 진실원. 모든 쓰기는 여기를 통과한다.
// 패널은 editor.layers() 읽기 전용, 쓰기는 apply()로만(이중 상태관리 금지).

export class Renderer {
  constructor(editor, canvas) {
    this.editor = editor;
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d", { alpha: true });
    this.ctx.imageSmoothingEnabled = false;
    this._dirty = false;
    this._raf = 0;
  }
  resize() {
    this.canvas.width = this.editor.width();
    this.canvas.height = this.editor.height();
    this.markDirty();
  }
  markDirty() {
    this._dirty = true;
    if (!this._raf) this._raf = requestAnimationFrame(() => this._frame());
  }
  _frame() {
    this._raf = 0;
    if (!this._dirty) return;
    this._dirty = false;
    // 매 프레임 새 복사본을 받는다(메모리 detach 무관 — wasm이 소유 복사본 반환).
    const buf = this.editor.composite_rgba();
    const img = new ImageData(buf, this.editor.width(), this.editor.height());
    this.ctx.putImageData(img, 0, 0);
  }
}

export class App extends EventTarget {
  constructor(editor, renderer) {
    super();
    this.editor = editor;
    this.renderer = renderer;
  }

  /** Action 배열을 적용하고 화면·패널을 갱신한다(단일 쓰기 funnel). */
  apply(actions) {
    const res = JSON.parse(this.editor.apply_actions(JSON.stringify(actions)));
    if (!res.ok) {
      console.error("apply 실패:", res.issues);
      this._notify();
      return res;
    }
    this.renderer.markDirty();
    this._notify();
    return res;
  }

  undo() {
    if (this.editor.undo()) {
      this.renderer.markDirty();
      this._notify();
    }
  }
  redo() {
    if (this.editor.redo()) {
      this.renderer.markDirty();
      this._notify();
    }
  }

  /** 레이어 목록(파생 뷰, top-to-bottom). */
  layers() {
    const j = JSON.parse(this.editor.layers());
    return j.layers.slice().reverse(); // bottom-to-top → 표시용 top-to-bottom
  }
  canUndo() { return this.editor.can_undo(); }
  canRedo() { return this.editor.can_redo(); }
  docInfo() { return JSON.parse(this.editor.doc_info()); }

  _notify() {
    this.dispatchEvent(new CustomEvent("changed"));
  }
}
