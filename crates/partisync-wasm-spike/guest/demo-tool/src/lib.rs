//! demo guest：最小 MCP-tool 语义（JSON 入参 → JSON 出参）。
//! 零 wasi import：不做 IO、不开线程、panic=abort，供宿主「默认拒权」linker 实例化。

wit_bindgen::generate!({
    world: "demo-tool",
    path: "wit",
});

struct DemoTool;

impl Guest for DemoTool {
    fn call(input: String) -> String {
        let value: serde_json::Value =
            serde_json::from_str(&input).unwrap_or(serde_json::Value::Null);
        serde_json::json!({ "echo": value, "input_bytes": input.len() }).to_string()
    }
}

export!(DemoTool);
