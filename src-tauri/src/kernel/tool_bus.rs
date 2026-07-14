// ToolBus — 统一 tool_call 协议 wrapper
//
// 设计原则:
//   - 1 tool = 1 原子动作 (生成图 / 生成视频 / 抽帧 / 查询 / 导出 / 读写记忆)
//   - 不替用户决策 — 每条 tool_call 都来自用户主动触发或 skill 注入
//   - skill 声明自己需要的 tools (W10 manifest schema 已建)
//   - tool_call 失败必须可重试, 不静默吞错
//
// 关联红线:
//   - 红线 7: Director skill 不能自动跑完 tool_call, 每步"我确认"后才执行
//   - 红线 9: tool:read_memory / tool:write_memory 必走 MemoryBus, 真记
//
// 当前 (Day 1-2) 只做协议层 (类型 + 调度接口), 不实现具体 tool.
// 具体 tool (generate_image 等) 在已有 image_adapter / video_adapter 里,
//   后续让它们实现 Tool trait 接入 ToolBus.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Tool 调用.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
}

/// Tool 执行结果.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ToolResult {
    Ok {
        call_id: String,
        output: serde_json::Value,
    },
    Err {
        call_id: String,
        message: String,
    },
}

impl ToolResult {
    pub fn ok(call_id: &str, output: serde_json::Value) -> Self {
        Self::Ok {
            call_id: call_id.to_string(),
            output,
        }
    }
    pub fn err(call_id: &str, message: impl Into<String>) -> Self {
        Self::Err {
            call_id: call_id.to_string(),
            message: message.into(),
        }
    }
    pub fn call_id(&self) -> &str {
        match self {
            Self::Ok { call_id, .. } | Self::Err { call_id, .. } => call_id,
        }
    }
}

/// Tool trait — 所有 tool 都实现这个 trait, 让 ToolBus 调度.
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    /// Args schema 描述, 前端可生成表单. Day 1-2 简单 JSON, 后续接 JSON Schema.
    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    /// 执行. 必须返回 Ok 或 Err, 不 panic 给上层.
    fn execute(&self, call_id: &str, args: serde_json::Value) -> ToolResult;
}

/// ToolBus — 注册表 + 调度.
#[derive(Clone)]
pub struct ToolBus {
    tools: Arc<RwLock<HashMap<String, Arc<dyn Tool>>>>,
}

impl ToolBus {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register(&self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.write().expect("tool bus lock").insert(name, tool);
    }

    pub fn list(&self) -> Vec<String> {
        self.tools
            .read()
            .expect("tool bus lock")
            .keys()
            .cloned()
            .collect()
    }

    /// 调度一次 tool_call.
    pub fn dispatch(&self, call: ToolCall) -> ToolResult {
        let tools = self.tools.read().expect("tool bus lock");
        match tools.get(&call.name) {
            Some(tool) => tool.execute(&call.id, call.args),
            None => ToolResult::err(&call.id, format!("tool not found: {}", call.name)),
        }
    }

    /// 批量调度. 失败的 tool 不阻塞其他 tool.
    pub fn dispatch_batch(&self, calls: Vec<ToolCall>) -> Vec<ToolResult> {
        calls.into_iter().map(|c| self.dispatch(c)).collect()
    }
}

impl Default for ToolBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoTool;
    impl Tool for EchoTool {
        fn name(&self) -> &'static str {
            "echo"
        }
        fn execute(&self, call_id: &str, args: serde_json::Value) -> ToolResult {
            ToolResult::ok(call_id, args)
        }
    }

    struct BoomTool;
    impl Tool for BoomTool {
        fn name(&self) -> &'static str {
            "boom"
        }
        fn execute(&self, call_id: &str, _args: serde_json::Value) -> ToolResult {
            ToolResult::err(call_id, "boom always fails")
        }
    }

    #[test]
    fn register_and_dispatch_ok() {
        let bus = ToolBus::new();
        bus.register(Arc::new(EchoTool));
        let r = bus.dispatch(ToolCall {
            id: "c1".into(),
            name: "echo".into(),
            args: serde_json::json!({"msg": "hi"}),
        });
        match r {
            ToolResult::Ok { call_id, output } => {
                assert_eq!(call_id, "c1");
                assert_eq!(output["msg"], "hi");
            }
            _ => panic!("expected Ok"),
        }
    }

    #[test]
    fn dispatch_unknown_tool_returns_err() {
        let bus = ToolBus::new();
        let r = bus.dispatch(ToolCall {
            id: "c2".into(),
            name: "ghost".into(),
            args: serde_json::json!({}),
        });
        match r {
            ToolResult::Err { call_id, message } => {
                assert_eq!(call_id, "c2");
                assert!(message.contains("not found"));
            }
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn dispatch_err_tool_returns_err() {
        let bus = ToolBus::new();
        bus.register(Arc::new(BoomTool));
        let r = bus.dispatch(ToolCall {
            id: "c3".into(),
            name: "boom".into(),
            args: serde_json::json!({}),
        });
        assert!(matches!(r, ToolResult::Err { .. }));
    }

    #[test]
    fn batch_dispatch_independent() {
        let bus = ToolBus::new();
        bus.register(Arc::new(EchoTool));
        bus.register(Arc::new(BoomTool));
        let results = bus.dispatch_batch(vec![
            ToolCall {
                id: "a".into(),
                name: "echo".into(),
                args: serde_json::json!({"k": 1}),
            },
            ToolCall {
                id: "b".into(),
                name: "boom".into(),
                args: serde_json::json!({}),
            },
            ToolCall {
                id: "c".into(),
                name: "echo".into(),
                args: serde_json::json!({"k": 2}),
            },
        ]);
        assert_eq!(results.len(), 3);
        assert!(matches!(results[0], ToolResult::Ok { .. }));
        assert!(matches!(results[1], ToolResult::Err { .. }));
        assert!(matches!(results[2], ToolResult::Ok { .. }));
    }

    #[test]
    fn list_returns_registered_tools() {
        let bus = ToolBus::new();
        bus.register(Arc::new(EchoTool));
        bus.register(Arc::new(BoomTool));
        let mut names = bus.list();
        names.sort();
        assert_eq!(names, vec!["boom", "echo"]);
    }
}