// Action 빌더 — dispatch.rs의 serde 형태와 1:1. png_path는 브라우저에 없음(fs).
//
// PixelSource(브라우저): {from:"transparent"} | {from:"fill",rgba:[r,g,b,a]}
//                       | {from:"png_base64",data} | {from:"shapes",items:[Shape]}
// Shape: {shape:"rect",x,y,w,h,rgba} | {shape:"ellipse",cx,cy,rx,ry,rgba}
//        | {shape:"line",x0,y0,x1,y1,width,rgba}
// NodeRef: {node:<id>} | {bind:"<name>"}

export const fill = (rgba) => ({ from: "fill", rgba });
export const transparent = () => ({ from: "transparent" });
export const shapes = (items) => ({ from: "shapes", items });
// base64(헤더 제외 순수 데이터) PNG → 레이어 소스. 문서 크기와 일치해야 엔진이 받는다.
export const pngBase64 = (data) => ({ from: "png_base64", data });

export const rect = (x, y, w, h, rgba) => ({ shape: "rect", x, y, w, h, rgba });
export const ellipse = (cx, cy, rx, ry, rgba) => ({ shape: "ellipse", cx, cy, rx, ry, rgba });
export const line = (x0, y0, x1, y1, width, rgba) => ({ shape: "line", x0, y0, x1, y1, width, rgba });
export const strokeRect = (x, y, w, h, width, rgba) => ({ shape: "stroke_rect", x, y, w, h, width, rgba });
export const strokeEllipse = (cx, cy, rx, ry, width, rgba) => ({ shape: "stroke_ellipse", cx, cy, rx, ry, width, rgba });
export const roundedRect = (x, y, w, h, radius, rgba) => ({ shape: "rounded_rect", x, y, w, h, radius, rgba });
export const strokeRoundedRect = (x, y, w, h, radius, width, rgba) => ({ shape: "stroke_rounded_rect", x, y, w, h, radius, width, rgba });
// 텍스트(번들 폰트 Pretendard, 한글/라틴). (x,y)=첫 줄 좌상단, size=px, '\n' 줄바꿈.
export const text = (x, y, content, size, rgba) => ({ shape: "text", x, y, text: content, size, rgba });

export const addPaintLayer = (name, source, opts = {}) => ({
  op: "add_paint_layer",
  name,
  source,
  ...(opts.index != null ? { index: opts.index } : {}),
  ...(opts.bind ? { bind: opts.bind } : {}),
});

export const setProps = (id, patch) => ({ op: "set_props", id: ref(id), patch });
// 픽셀 소스 교체(노드 id·그룹 소속·z순서·선택 보존) — 재스타일/재래스터 전용.
export const replacePaintSource = (id, source) => ({ op: "replace_paint_source", id: ref(id), source });
export const setBlend = (id, mode) => ({ op: "set_blend", id: ref(id), mode });
// 캔버스 평행이동(절대 offset, [dx,dy]). Move 툴이 사용.
export const setOffset = (id, offset) => ({ op: "set_props", id: ref(id), patch: { offset } });
export const moveLayer = (id, to) => ({ op: "move_layer", id: ref(id), to });
export const deleteLayer = (id) => ({ op: "delete_layer", id: ref(id) });
// 레이어 복제(표면+속성 복사, offset +12px).
export const duplicateLayer = (id) => ({ op: "duplicate_layer", id: ref(id) });
// 비파괴 트랜스폼(표면 중심 기준).
export const setScale = (id, scale) => ({ op: "set_props", id: ref(id), patch: { scale } });
export const setRotation = (id, rotation) => ({ op: "set_props", id: ref(id), patch: { rotation } });

// 숫자 id → {node}, 문자열 → {bind}.
function ref(id) {
  return typeof id === "string" ? { bind: id } : { node: id };
}
// 자유곡선(브러시) — points = [x0,y0,x1,y1,...].
export const path = (points, width, rgba) => ({ shape: "path", points, width, rgba });
// 그룹 묶기/해제.
export const groupLayers = (ids, name = "group") => ({ op: "group_layers", ids: ids.map(ref), name });
export const ungroup = (id) => ({ op: "ungroup", id: ref(id) });
// Frame 목록 전체 교체(무한 작업영역의 export 단위).
export const setFrames = (frames) => ({ op: "set_frames", frames });
