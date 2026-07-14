// 关键词审核（W2.7 + W11 Day 8 双 profile）
//
// 极简实现：黑白名单 + 长度限制
// 关键设计：审核在 Agent Loop 入口（用户输入）+ 出口（AI 输出）各跑一次
//
// W11 Day 8 — Part C 双 profile 路由:
//   SafetyProfile::Child — 强审核（黑名单 + 白名单 + 模糊 + 长度）
//   SafetyProfile::Adult — 极简（仅拦截极端 illegal 词：CSAM / 真实暴力威胁 / doxxing）
// 切换: switch_profile(mode) 调用方 (UserMode IPC) 触发
// 默认: Child (与历史行为兼容 — 不破坏现有 230 测试)

use crate::license_store::UserMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SafetyVerdict {
    Pass,
    Warn { reason: String },
    Block { reason: String },
}

/// W11 Day 8: 双 profile — 儿童模式严格, 成人模式仅极端 illegal 拦截
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyProfile {
    /// 儿童模式 (默认): 黑/白名单 + 模糊 + 长度 + 商业广告词拦截
    Child,
    /// 成人模式: 仅留 extreme illegal 类 (暴力威胁 CSAM doxxing)
    Adult,
}

impl SafetyProfile {
    pub fn for_mode(mode: UserMode) -> Self {
        match mode {
            UserMode::Child => SafetyProfile::Child,
            UserMode::Adult => SafetyProfile::Adult,
        }
    }
}

/// 关键词过滤
pub struct KeywordFilter {
    profile: SafetyProfile,
    /// 完全屏蔽的词 (按 profile 切换不同列表)
    blocked: Vec<&'static str>,
    /// 警告但不屏蔽的词 (儿童模式有效, 成人模式空)
    warn: Vec<&'static str>,
    /// 最大输入长度 (成人模式显著放宽)
    max_len: usize,
}

impl KeywordFilter {
    pub fn new() -> Self {
        // 默认 Child profile — 不破坏现有 26 个老单测
        Self::for_profile(SafetyProfile::Child)
    }

    /// 按 profile 构造 — Day 8 新增入口
    pub fn for_profile(profile: SafetyProfile) -> Self {
        match profile {
            SafetyProfile::Child => Self {
                profile,
                blocked: CHILD_BLOCKED.to_vec(),
                warn: CHILD_WARN.to_vec(),
                max_len: 5_000,
            },
            SafetyProfile::Adult => Self {
                profile,
                // 成人模式仅拦截极端 illegal — 不查商业/广告/暴力等
                blocked: ADULT_BLOCKED_MINIMAL.to_vec(),
                warn: Vec::new(),
                max_len: 32_000,
            },
        }
    }

    /// 当前 profile
    pub fn profile(&self) -> SafetyProfile {
        self.profile
    }

    /// W11 Day 8: 切换 profile (mode switch 时调用)
    pub fn switch_profile(&mut self, mode: UserMode) {
        let target = SafetyProfile::for_mode(mode);
        if target == self.profile {
            return;
        }
        self.profile = target;
        match target {
            SafetyProfile::Child => {
                self.blocked = CHILD_BLOCKED.to_vec();
                self.warn = CHILD_WARN.to_vec();
                self.max_len = 5_000;
            }
            SafetyProfile::Adult => {
                self.blocked = ADULT_BLOCKED_MINIMAL.to_vec();
                self.warn.clear();
                self.max_len = 32_000;
            }
        }
    }

    pub fn check(&self, text: &str) -> SafetyVerdict {
        let lower = text.to_lowercase();
        for w in &self.blocked {
            if lower.contains(w) {
                return SafetyVerdict::Block {
                    reason: format!("包含敏感词：{}", w),
                };
            }
        }
        for w in &self.warn {
            if lower.contains(w) {
                return SafetyVerdict::Warn {
                    reason: format!("检测到情绪词：{}，建议 AI 引导孩子表达", w),
                };
            }
        }
        if text.chars().count() > self.max_len {
            return SafetyVerdict::Block {
                reason: format!("内容超过 {} 字", self.max_len),
            };
        }
        SafetyVerdict::Pass
    }
}

/// W11 Day 8: 儿童 profile 黑名单 — 包含所有原 hardcode (向后兼容老测试)
const CHILD_BLOCKED: &[&str] = &[
    "blood",
    "kill",
    "gun",
    "porn",
    "裸",
    "色情",
    "毒品",
    "自杀",
    "炸弹",
    "weapon",
    "drugs",
    // 商业广告词 — 仅儿童模式拦 (娃看到带货话术易混淆)
    "commercial",
    "advertising",
    "带货",
    "广告位",
];

/// W11 Day 8: 儿童 profile 警告词 (情绪)
const CHILD_WARN: &[&str] = &["怕", "讨厌", "恨", "生气"];

/// W11 Day 8: 成人 profile 极简拦截 — 仅 illegal / 极端暴力 / doxxing
/// 注: 真实部署应含更复杂的 illegal 内容识别; Day 8 仅提供最小可用集
const ADULT_BLOCKED_MINIMAL: &[&str] = &[
    // CSAM (永远拦截)
    "csam",
    // 真实暴力威胁 (个人级)
    "我要杀了你",
    "找你报仇",
    // doxxing
    "dox",
    "人肉搜索",
];

impl Default for KeywordFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_normal_text() {
        let f = KeywordFilter::new();
        assert_eq!(f.check("一只小猫在月光下追蝴蝶"), SafetyVerdict::Pass);
    }

    #[test]
    fn block_sensitive_word() {
        let f = KeywordFilter::new();
        assert!(matches!(f.check("我想看 gun"), SafetyVerdict::Block { .. }));
    }

    #[test]
    fn warn_emotion_word() {
        let f = KeywordFilter::new();
        assert!(matches!(f.check("我讨厌数学"), SafetyVerdict::Warn { .. }));
    }

    #[test]
    fn block_too_long() {
        let f = KeywordFilter::new();
        let long = "啊".repeat(6000);
        assert!(matches!(f.check(&long), SafetyVerdict::Block { .. }));
    }

    // ============ W11 Day 8: 双 profile 测试 ============

    #[test]
    fn child_blocks_adult_keywords() {
        let f = KeywordFilter::for_profile(SafetyProfile::Child);
        // 商业广告词在儿童模式下应被拦截
        let v = f.check("今天给大家带货");
        assert!(
            matches!(v, SafetyVerdict::Block { .. }),
            "儿童模式应拦截 '带货', got {:?}",
            v
        );
        let v = f.check("这是一段 advertising 文案");
        assert!(
            matches!(v, SafetyVerdict::Block { .. }),
            "儿童模式应拦截 'advertising', got {:?}",
            v
        );
    }

    #[test]
    fn adult_allows_commercial_keywords() {
        let f = KeywordFilter::for_profile(SafetyProfile::Adult);
        // 商业广告词在成人模式下放行
        assert_eq!(f.check("今天给大家带货"), SafetyVerdict::Pass);
        assert_eq!(f.check("一段 advertising 文案"), SafetyVerdict::Pass);
        // 暴力词在成人模式下也放行 (除非极端)
        assert_eq!(f.check("gun illustration for design"), SafetyVerdict::Pass);
    }

    #[test]
    fn adult_still_blocks_extreme_illegal() {
        let f = KeywordFilter::for_profile(SafetyProfile::Adult);
        // CSAM 永远拦截
        assert!(matches!(f.check("csam content"), SafetyVerdict::Block { .. }));
        // doxxing 拦截
        assert!(matches!(f.check("dox him"), SafetyVerdict::Block { .. }));
        assert!(matches!(
            f.check("人肉搜索那位同学"),
            SafetyVerdict::Block { .. }
        ));
    }

    #[test]
    fn switch_profile_updates_keywords() {
        let mut f = KeywordFilter::new(); // 默认 Child
                                  // 儿童 → 带货拦截
        assert!(matches!(
            f.check("今天给大家带货"),
            SafetyVerdict::Block { .. }
        ));
        // 切到成人
        f.switch_profile(UserMode::Adult);
        assert_eq!(f.check("今天给大家带货"), SafetyVerdict::Pass);
        // 切回儿童
        f.switch_profile(UserMode::Child);
        assert!(matches!(
            f.check("今天给大家带货"),
            SafetyVerdict::Block { .. }
        ));
    }

    #[test]
    fn adult_profile_has_higher_max_len() {
        let child = KeywordFilter::for_profile(SafetyProfile::Child);
        let adult = KeywordFilter::for_profile(SafetyProfile::Adult);
        assert!(adult.max_len > child.max_len);
        // 5000 字对儿童太长, 对成人仍 ok
        let long = "x".repeat(8000);
        assert!(matches!(
            child.check(&long),
            SafetyVerdict::Block { .. }
        ));
        assert_eq!(adult.check(&long), SafetyVerdict::Pass);
    }

    #[test]
    fn profile_for_mode_routes_correctly() {
        assert_eq!(
            SafetyProfile::for_mode(UserMode::Child),
            SafetyProfile::Child
        );
        assert_eq!(
            SafetyProfile::for_mode(UserMode::Adult),
            SafetyProfile::Adult
        );
    }

    #[test]
    fn switch_profile_idempotent() {
        let mut f = KeywordFilter::new();
        let _ = f.profile();
        // 重复切到同一个 profile → 不 panic 不重复
        f.switch_profile(UserMode::Child);
        f.switch_profile(UserMode::Child);
        assert_eq!(f.profile(), SafetyProfile::Child);
    }
}
