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

export const addPaintLayer = (name, source, opts = {}) => ({
  op: "add_paint_layer",
  name,
  source,
  ...(opts.index != null ? { index: opts.index } : {}),
  ...(opts.bind ? { bind: opts.bind } : {}),
});

export const setProps = (id, patch) => ({ op: "set_props", id: ref(id), patch });
export const setBlend = (id, mode) => ({ op: "set_blend", id: ref(id), mode });
// 캔버스 평행이동(절대 offset, [dx,dy]). Move 툴이 사용.
export const setOffset = (id, offset) => ({ op: "set_props", id: ref(id), patch: { offset } });
export const moveLayer = (id, to) => ({ op: "move_layer", id: ref(id), to });
export const deleteLayer = (id) => ({ op: "delete_layer", id: ref(id) });

// 숫자 id → {node}, 문자열 → {bind}.
function ref(id) {
  return typeof id === "string" ? { bind: id } : { node: id };
}
