// kernel/state.rs — Kernel 单例 (EventBus + MemoryBus + IdentityService)
// 整体作为 Tauri State 注入. 与其他 managed service (LicenseStore / SkillsState) 同模式.
//
// 设计:
//   - 启动期 setup() 一次性实例化, 之后只读共享 (Arc).
//   - 不在 IPC handler 内 new IdentityService, 否则会出现多份 EventBus, PetMoodChanged 事件飞不到订阅者.
//   - 失败不阻塞启动: backend 打开失败 → 走 fallback (返回 None); 但 EventBus 必须有 (供 telemetry 上报).

use std::path::Path;
use std::sync::Arc;

use crate::elog;
use crate::kernel::event_bus::EventBus;
use crate::kernel::identity::IdentityService;
use crate::kernel::memory_bus::MemoryBus;
use crate::kernel::memory_store::SqliteMemoryBackend;

/// Kernel 单例 — IPC handler 通过 app.state::<KernelState>() 取.
pub struct KernelState {
    pub event_bus: EventBus,
    pub identity_svc: IdentityService,
    pub backend_kind: BackendKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Sqlite,
    /// 打开失败 fallback — 仅 EventBus 活着, 所有 memory 操作返 None / noop.
    /// 启动期不阻塞, 但 PetEngine 等会拿到空 identity, 自动 NoOp.
    Unavailable,
}

impl KernelState {
    /// 在 setup 闭包内调用, data_dir/app_data_dir/kernel.sqlite 路径.
    /// 失败也不 panic — 返回 Unavailable, 启动照常完成.
    pub fn bootstrap(data_dir: &Path) -> Self {
        let event_bus = EventBus::new();
        let db_path = data_dir.join("kernel.sqlite");
        let backend_result = SqliteMemoryBackend::open(&db_path).map(Arc::new);
        match backend_result {
            Ok(backend) => {
                let mem = MemoryBus::new(backend, event_bus.clone());
                let identity_svc = IdentityService::new(mem, event_bus.clone());
                elog!(
                    "[kernel] state initialized at {:?} (sqlite, event_bus ready)",
                    db_path
                );
                Self {
                    event_bus,
                    identity_svc,
                    backend_kind: BackendKind::Sqlite,
                }
            }
            Err(e) => {
                eprintln!("[kernel] failed to open kernel.sqlite at {db_path:?}: {e}");
                eprintln!("[kernel] falling back to Unavailable — pet engine will NoOp");
                // 仍保留一个空的 MemoryBus (InMemory) + IdentityService 持有它;
                // 这样 IPC 调用不崩, 只是 identity_svc.load() 永远 None.
                let backend = Arc::new(InMemoryOnlyBackend) as Arc<dyn crate::kernel::memory_bus::MemoryBackend>;
                let mem = MemoryBus::new(backend, event_bus.clone());
                let identity_svc = IdentityService::new(mem, event_bus.clone());
                Self {
                    event_bus,
                    identity_svc,
                    backend_kind: BackendKind::Unavailable,
                }
            }
        }
    }

    /// 给前端读当前 pet_mood. identity 不存在时返 happy 默认 (前端可独立展示).
    pub fn current_pet_mood(&self, user_id: &str) -> String {
        self.identity_svc
            .load(user_id)
            .map(|i| i.pet_mood)
            .unwrap_or_else(|| "happy".to_string())
    }
}

/// 启动期 fallback 用的 InMemory 实现, 跟 memory_bus.rs 里 InMemoryBackend 等价.
struct InMemoryOnlyBackend;
impl crate::kernel::memory_bus::MemoryBackend for InMemoryOnlyBackend {
    fn get(&self, _: &str, _: &str) -> Option<String> {
        None
    }
    fn put(&self, _: &str, _: &str, _: &str) {}
    fn append(&self, _: &str, _: &str, _: &str) {}
    fn list(&self, _: &str) -> Vec<String> {
        Vec::new()
    }
}