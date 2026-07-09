// 关键词审核（W2.7）
// 极简实现：黑白名单 + 长度限制
// 真实实现会替换为：内容安全 API（阿里云 / 腾讯云 / 自建审核）
// 关键设计：审核在 Agent Loop 入口（用户输入）+ 出口（AI 输出）各跑一次

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SafetyVerdict {
    Pass,
    Warn { reason: String },
    Block { reason: String },
}

/// 简单关键词过滤（MVP 阶段足够 demo）
/// 真实系统会调用内容安全 API + 上下文判断
pub struct KeywordFilter {
    /// 完全屏蔽的词（如：暴力、色情、危险行为）
    blocked: Vec<&'static str>,
    /// 警告但不屏蔽的词（需要 AI 引导孩子换个方向）
    warn: Vec<&'static str>,
    /// 最大输入长度（防止 prompt 注入）
    max_len: usize,
}

impl KeywordFilter {
    pub fn new() -> Self {
        Self {
            blocked: vec![
                "blood", "kill", "gun", "porn", "裸", "色情", "毒品", "自杀", "炸弹",
                "weapon", "drugs",
            ],
            warn: vec![
                "怕", "讨厌", "恨", "生气",
            ],
            // 真实 LLM 回答会带教学说明，500 字太短容易误伤
            max_len: 5000,
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
}
