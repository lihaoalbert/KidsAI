// Agent 内核 — Layer 1
// 通用共创 agent 的最小可信核心:
//   1. EventBus      — 异步广播, 解耦 skill ↔ shell ↔ kernel
//   2. MemoryBus     — 跨 namespace 持久读写 (语义层)
//   3. ToolBus       — tool_call 协议 wrapper (1 tool = 1 原子动作)
//   4. IdentitySvc   — 用户 + 宠物 + parent_id
//   5. SkillLoader   — 按 audience + mode 过滤 + UI mount
//   6. PetEngine     — 宠物情绪 + 主动消息调度
//   7. SafetyBus     — 按 mode 路由 (W11 已有, 这里留 hook)
//   8. ModeBus       — 模式切换总线 (W11 已有)
//
// 红线 (来自 4-persona 沙盘):
//   - 红线 7: 每步都有"我确认"按钮 → 由 skill 各自 UI 保证, 内核不替用户决策
//   - 红线 8: L1-L7 删除后必须补 skill 钩子 → SkillLoader 严格校验
//   - 红线 9: MemoryStore 必须真记 → MemoryBus 强一致

pub mod event_bus;
pub mod identity;
pub mod ipc;
pub mod lesson_templates;
pub mod memory_bus;
pub mod memory_store;
pub mod pet_engine;
pub mod seed_skills;
pub mod skill_loader;
pub mod state;
pub mod tool_bus;

pub use event_bus::{EventBus, KernelEvent};
pub use memory_bus::{MemoryBus, MemoryOp};
pub use tool_bus::{ToolBus, ToolCall, ToolResult};