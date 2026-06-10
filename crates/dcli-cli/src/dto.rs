//! JSON DTO — CLI(`--json`)와 MCP tool 결과가 공유하는 **단일 진실원**.
//!
//! "CLI verb ≡ MCP tool"이 같은 payload를 내도록, 노드/문서의 JSON 셰이프를 여기서만
//! 정의한다. CLI output.rs의 Emitter와 dcli-mcp tool 핸들러가 모두 이 함수를 호출하므로
//! 두 표면의 JSON이 컴파일러가 강제하는 단일 코드경로로 일치한다(검증 #2 단일 진실원).

use dcli_model::{BlendMode, Document, Node, NodeKind};
use serde_json::{json, Value};

pub fn node_kind_str(node: &Node) -> &'static str {
    match node.kind {
        NodeKind::Paint { .. } => "paint",
        NodeKind::Group => "group",
    }
}

/// 블렌드 모드를 snake_case 문자열로(set_blend Action과 동일 케이싱 → 매핑 불필요).
pub fn blend_str(blend: BlendMode) -> &'static str {
    match blend {
        BlendMode::Normal => "normal",
        BlendMode::Multiply => "multiply",
        BlendMode::Screen => "screen",
    }
}

/// 단일 노드 JSON.
pub fn node_json(node: &Node) -> Value {
    json!({
        "id": node.id.0,
        "name": node.name,
        "kind": node_kind_str(node),
        "visible": node.visible,
        "opacity": node.opacity,
        "blend": blend_str(node.blend),
        "offset": [node.offset.0, node.offset.1],
        "scale": [node.scale.0, node.scale.1],
        "rotation": node.rotation,
        "surface": node.surface_id().map(|s| s.0),
    })
}

/// 문서 메타 JSON(희소, 인지 루프 1단계).
pub fn doc_info_json(doc: &Document) -> Value {
    json!({
        "w": doc.width,
        "h": doc.height,
        "depth": format!("{:?}", doc.bit_depth),
        "blend_space": format!("{:?}", doc.blend_space),
        "layers": doc.node_count(),
    })
}

/// bottom-to-top 레이어 목록 JSON(인지 루프 2단계).
pub fn layer_list_json(doc: &Document) -> Value {
    let arr: Vec<Value> = doc.iter_bottom_to_top().map(node_json).collect();
    json!({ "layers": arr })
}

/// 사람용 한 줄 노드 요약.
pub fn node_human(node: &Node) -> String {
    format!(
        "n{} \"{}\" [{}] {} opacity={:.2} {:?}",
        node.id.0,
        node.name,
        node_kind_str(node),
        if node.visible { "visible" } else { "hidden" },
        node.opacity,
        node.blend,
    )
}
