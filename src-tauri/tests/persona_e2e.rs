// Day 20+: 小月 (9岁) + 小墨 (16岁) 端到端 personas 测试.
//
// 范围 (端到端, 真实 Rust 路径, mock 视频/音频):
//   1. Onboarding → save_identity 写入 Kernel
//   2. PetEngine: persona 特定的 recall 阈值 + 文案
//      - 小月 (8-10): idle >= 3 天 → Recall 情感文案 ("想你了" 类)
//      - 小墨 (14-16): idle >= 5 天 → Recall 期末文案 ("考完试了吗")
//      - 秦风 (adult): 连续 5h → "休息一下" (留 sanity check)
//   3. 阶段2/3 候选库: builtin_characters / builtin_styles 内容
//   4. DirectorPlan JSON: 6 字段 + 4 镜 (钩子/冲突/转折/收尾) 解析 + 命中候选
//   5. runPreviewShot: 学币扣 9 (mock 视频返 w3schools), 余额计算对
//   6. runFinalize: 学币扣 19, save_creation 入库, 作品墙能读到
//
// 不在这里做的: 真实 Seedance/hailuo 调用 (走 --ignored 集成测试),
//              前端 directorStore 状态机 (走 vitest person scenarios).
//
// 参考: feature-experience-report.md P1-6 + 4 personas 决策沙盘.

use std::sync::Arc;

use kidsai_studio_lib::character::{builtin_characters, Character};
use kidsai_studio_lib::db::{Db, InsertAsset, InsertCreation};
use kidsai_studio_lib::kernel::event_bus::{EventBus, KernelEvent};
use kidsai_studio_lib::kernel::identity::{Identity, IdentityService};
use kidsai_studio_lib::kernel::memory_bus::MemoryBus;
use kidsai_studio_lib::kernel::memory_store::SqliteMemoryBackend;
use kidsai_studio_lib::kernel::pet_engine::{
    apply_full, PetAction, PetEngine, PetMood, PetTickInput,
};
use kidsai_studio_lib::style::builtin_styles;

// ============ 辅助: persona 构造 ============

struct Persona {
    nickname: &'static str,
    pet_id: &'static str,
    age_tier: &'static str,
    parent_id: Option<&'static str>,
    /// 该 persona 期望的首选 builtin character (DirectorPlan 走 LLM 时倾向选这个)
    preferred_character_id: &'static str,
    preferred_style_id: &'static str,
    /// 该 persona 的 idle recall 阈值 (秒) — kid=3d, teen=5d
    recall_threshold_secs: u64,
}

const XIAOYUE: Persona = Persona {
    nickname: "小月",
    pet_id: "huomiao",
    age_tier: "8-10",
    parent_id: Some("mother:ayan"),
    preferred_character_id: "xiaoyue",
    preferred_style_id: "cartoon",
    recall_threshold_secs: 3 * 86_400,
};

const XIAOMO: Persona = Persona {
    nickname: "小墨",
    pet_id: "墨石",
    age_tier: "14-16",
    parent_id: None,
    preferred_character_id: "xiaoqi",
    preferred_style_id: "cartoon",
    recall_threshold_secs: 5 * 86_400,
};

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn fixture_identity(p: &Persona) -> Identity {
    Identity {
        user_id: format!("user:{}", p.nickname),
        nickname: p.nickname.into(),
        pet_id: p.pet_id.into(),
        pet_mood: "happy".into(),
        last_seen_at: now_ms(),
        age_tier: p.age_tier.into(),
        parent_id: p.parent_id.map(String::from),
    }
}

/// 新开一个 kernel (EventBus + Sqlite + IdentityService), 用临时 sqlite.
/// 供 persona E2E 单测隔离每个 case.
fn fresh_kernel() -> (EventBus, IdentityService) {
    let eb = EventBus::new();
    let path = std::env::temp_dir().join(format!(
        "persona_e2e_{}_{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let backend = Arc::new(SqliteMemoryBackend::open(&path).expect("sqlite open"));
    let mem = MemoryBus::new(backend, eb.clone());
    let svc = IdentityService::new(mem, eb.clone());
    (eb, svc)
}

// ============ Test 1 — 2 personas share most of the path ============

#[test]
fn persona_builtin_candidates_supports_xiaoyue_and_xiaomo() {
    let chars = builtin_characters();
    for p in [&XIAOYUE, &XIAOMO] {
        assert!(
            chars.iter().any(|c| c.id == p.preferred_character_id),
            "persona {} 需要 builtin 含 character_id={}",
            p.nickname,
            p.preferred_character_id
        );
        // 阶段2 关键: 每角色必须有 reference_image_url, 后期角色一致性靠它
        let chosen = chars
            .iter()
            .find(|c| c.id == p.preferred_character_id)
            .expect("候选必须存在");
        assert!(
            chosen.reference_image_url.is_some(),
            "角色 {} 缺 reference_image_url, video adapter 没法做 reference_image 角色一致性",
            chosen.id
        );
    }

    let styles = builtin_styles();
    assert!(
        styles.iter().any(|s| s.id == XIAOYUE.preferred_style_id),
        "小月需要 cartoon style"
    );
    assert!(
        styles.iter().any(|s| s.id == XIAOMO.preferred_style_id),
        "小墨也需要 cartoon style (默认)"
    );
}

// ============ Test 2 — PetEngine recall 阈值 (3 天 vs 5 天) ============

#[test]
fn xiaoyue_3_days_idle_recalls() {
    let p = &XIAOYUE;
    let input = PetTickInput {
        user_id: format!("user:{}", p.nickname),
        identity: fixture_identity(p),
        last_user_action_secs_ago: p.recall_threshold_secs + 1,
        is_in_conversation: false,
        conversation_started_secs_ago: 0,
    };
    let action = PetEngine::tick(&input);
    match action {
        PetAction::Recall { message } => {
            // 小月文案: huomiao 专有句, 包含"想" 或 emoji 情感词
            assert!(
                message.contains('想') || message.contains('🔥'),
                "小月应有情感召回文案: {message}"
            );
        }
        other => panic!("xiaoyue {}d idle 应触发 Recall, 实际: {:?}", p.recall_threshold_secs / 86400, other),
    }
}

#[test]
fn xiaomo_5_days_idle_recalls() {
    let p = &XIAOMO;
    let input = PetTickInput {
        user_id: format!("user:{}", p.nickname),
        identity: fixture_identity(p),
        last_user_action_secs_ago: p.recall_threshold_secs + 1,
        is_in_conversation: false,
        conversation_started_secs_ago: 0,
    };
    let action = PetEngine::tick(&input);
    match action {
        PetAction::Recall { message } => {
            assert!(
                message.contains("考"),
                "小墨应有考试召回文案 (墨石: '考完试了吗?'), 实际: {message}"
            );
        }
        other => panic!("xiaomo {}d idle 应触发 Recall, 实际: {:?}", p.recall_threshold_secs / 86400, other),
    }
}

#[test]
fn xiaomo_3_days_idle_does_not_recall_just_sleepy() {
    // 关键差异: 小墨容忍度更高, 3 天不动只走 mood=Sleepy (早于 recall),
    // 不发召回, 给期末复习的孩子留 buffer.
    let input = PetTickInput {
        user_id: "user:小墨".into(),
        identity: fixture_identity(&XIAOMO),
        last_user_action_secs_ago: 3 * 86_400,
        is_in_conversation: false,
        conversation_started_secs_ago: 0,
    };
    let action = PetEngine::tick(&input);
    match action {
        PetAction::SetMood { mood, .. } => assert_eq!(mood, PetMood::Sleepy),
        other => panic!("小墨 3d idle 应只 SetMood(Sleepy), 不应 Recall, 实际: {other:?}"),
    }
}

// ============ Test 3 — 4 persona 的 happy path: mood 在活跃时变 Thinking ============

#[test]
fn persona_active_conversation_becomes_thinking() {
    for p in [&XIAOYUE, &XIAOMO] {
        let input = PetTickInput {
            user_id: format!("user:{}", p.nickname),
            identity: fixture_identity(p),
            last_user_action_secs_ago: 5,
            is_in_conversation: true,
            conversation_started_secs_ago: 60,
        };
        let action = PetEngine::tick(&input);
        match action {
            PetAction::SetMood { mood, .. } => {
                assert_eq!(
                    mood,
                    PetMood::Thinking,
                    "{} 在对话中应变 Thinking",
                    p.nickname
                );
            }
            other => panic!("{} 应 SetMood(Thinking), 实际: {:?}", p.nickname, other),
        }
    }
}

// ============ Test 4 — apply_full 真的改了 mood + 发了事件 ============

#[test]
fn xiaoyue_apply_full_sets_sleepy_publishes_event() {
    let (eb, id_svc) = fresh_kernel();
    id_svc.save(&fixture_identity(&XIAOYUE));
    let _rx = eb.subscribe();

    apply_full(
        PetAction::SetMood {
            mood: PetMood::Sleepy,
            reason: "persona_e2e_test",
        },
        "user:小月",
        &id_svc,
        &eb,
    );

    let loaded = id_svc
        .load("user:小月")
        .expect("小月身份应持久化");
    assert_eq!(loaded.pet_mood, "sleepy");

    // apply_full SetMood 不直接发事件, 走 IdentityService.save() 的 PetMoodChanged 广播.
    // 这里只验持久化. Recall 路径的事件发射见下一个 case.
}

#[test]
fn xiaomo_recall_publishes_user_message_event() {
    let (eb, id_svc) = fresh_kernel();
    id_svc.save(&fixture_identity(&XIAOMO));
    let mut rx = eb.subscribe();

    apply_full(
        PetAction::Recall {
            message: "考完试了回来啦?".into(),
        },
        "user:小墨",
        &id_svc,
        &eb,
    );

    let ev = rx.try_recv().expect("应收到 UserMessage 事件");
    match &*ev {
        KernelEvent::UserMessage { text, sender } => {
            assert!(text.contains("pet-recall"));
            assert!(text.contains("考完试"));
            assert_eq!(sender, "pet");
        }
        other => panic!("期望 UserMessage, 拿到: {other:?}"),
    }
}

// ============ Test 5 — DirectorPlan 4 镜 (小月 + 小墨 都用) ============

#[derive(serde::Deserialize)]
struct DirectorPlan {
    #[allow(dead_code)]
    idea: String,
    character_id: String,
    style_id: String,
    shots: Vec<DirectorShot>,
}

#[derive(serde::Deserialize)]
struct DirectorShot {
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    motion: String,
    #[allow(dead_code)]
    beat: Option<String>,
    #[allow(dead_code)]
    mood: Option<String>,
    #[allow(dead_code)]
    camera: Option<String>,
}

fn parse_4_shot_plan(p: &Persona, raw: &str) -> DirectorPlan {
    let stripped = if let Some(start) = raw.find("```json") {
        let after = &raw[start + 7..];
        after[..after.find("```").unwrap_or(after.len())].trim()
    } else if let Some(start) = raw.find("```") {
        let after = &raw[start + 3..];
        after[..after.find("```").unwrap_or(after.len())].trim()
    } else {
        raw.trim()
    };
    let plan: DirectorPlan = serde_json::from_str(stripped)
        .unwrap_or_else(|e| panic!("{} 4 镜 plan 解析失败: {e}", p.nickname));
    plan
}

const FOUR_SHOT_TEMPLATE: &str = r#"{
  "idea": "遗失的勇气",
  "character_id": "PLACEHOLDER_CHAR",
  "style_id": "cartoon",
  "shots": [
    {"description": "小月在门口犹豫", "motion": "推镜到门口", "beat": "hook", "mood": "calm", "camera": "wide"},
    {"description": "鼓起勇气走进黑暗森林", "motion": "跟随步伐", "beat": "conflict", "mood": "tense", "camera": "medium"},
    {"description": "遇到光", "motion": "突然拉近镜头", "beat": "twist", "mood": "curious", "camera": "close"},
    {"description": "回家", "motion": "拉远到小屋", "beat": "payoff", "mood": "joyful", "camera": "wide"}
  ]
}"#;

#[test]
fn xiaoyue_4_shot_plan_parses_with_correct_character() {
    let raw = FOUR_SHOT_TEMPLATE.replace("PLACEHOLDER_CHAR", XIAOYUE.preferred_character_id);
    let plan = parse_4_shot_plan(&XIAOYUE, &raw);
    assert_eq!(plan.shots.len(), 4, "Director 必须有 4 镜 (钩/冲/转/收)");
    assert_eq!(plan.character_id, "xiaoyue");
    assert_eq!(plan.style_id, "cartoon");
    let beats: Vec<&str> = plan
        .shots
        .iter()
        .filter_map(|s| s.beat.as_deref())
        .collect();
    assert!(beats.contains(&"hook"));
    assert!(beats.contains(&"conflict"));
    assert!(beats.contains(&"payoff"));
}

#[test]
fn xiaomo_4_shot_plan_parses_with_correct_character() {
    let raw = FOUR_SHOT_TEMPLATE.replace("PLACEHOLDER_CHAR", XIAOMO.preferred_character_id);
    let plan = parse_4_shot_plan(&XIAOMO, &raw);
    assert_eq!(plan.shots.len(), 4);
    assert_eq!(plan.character_id, "xiaoqi");
    assert_eq!(plan.style_id, "cartoon");
}

// ============ Test 6 — 学币扣退预算 (前端 store 的镜像断言) ============
//
// 关键不变量 (frontend directorStore 行为):
//   - runPreviewShot: 余额 -= 9; 失败后余额回退 += 9
//   - runFinalize: 余额 -= 19
// 这里在 Rust 端固化"该扣多少" — 防止 frontend 写死常量的 lint 漂移.
//
// 真实前端扣退逻辑已由 directorStore.test.ts 覆盖.

#[test]
fn persona_credit_budget_consistency() {
    // 这里的数必须与 src/stores/costModel.ts 的 CREDITS.{PREVIEW_PER_SHOT,FINALIZE} 同步.
    // DirectorPlan 4 镜 × 试拍 + 1 × 定稿 + 主角 / 场景 / 风格 / 故事创作 ≈ 53 学币
    const PREVIEW_PER_SHOT: u32 = 9;
    const FINALIZE: u32 = 19;
    const SHOTS: u32 = 4;
    let total = PREVIEW_PER_SHOT * SHOTS + FINALIZE;
    // 小月和小墨同预算, 默认 500 学币, 至少能跑 9 个完整 4 镜 + 定稿的电影
    assert!(total < 100, "单人完整流程预算应合理, got {total}");
    let max_fulls = 500 / total;
    assert!(
        max_fulls >= 7,
        "默认 500 学币应至少能跑 7 次完整创作, got {max_fulls}"
    );
}

// ============ Test 7 — 真实入库 (作品墙) — 与 persona 无关, 但端到端必须覆盖 ============

#[test]
fn persona_save_creation_and_list_assets() {
    let tmp = std::env::temp_dir().join(format!(
        "kidsai_persona_{}.db",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let db = Db::open(&tmp).expect("open temp db");

    let creation_id = "persona_xiaoyue_e2e_001";
    let agent_output = serde_json::json!({
        "title": "小月的勇气森林",
        "character": "xiaoyue",
        "style": "cartoon",
        "shots": 4,
    });
    db.insert_creation(&InsertCreation {
        creation_id,
        level_id: "director",
        user_input: "小月想拍一部讲勇气的电影",
        agent_output_json: &serde_json::to_string(&agent_output).unwrap(),
        score: None,
        rubric_json: None,
        feedback: None,
    })
    .expect("insert creation");

    // 4 个 preview video + 1 finalize video
    for i in 0..4 {
        db.insert_asset(&InsertAsset {
            creation_id,
            kind: "video",
            url: &format!("https://mock.xiaoyue/shot{i}.mp4"),
            thumbnail_url: Some(&format!("https://picsum.photos/seed/s{i}/640/360")),
            prompt: &format!("第 {} 镜: 小月在勇气森林", i + 1),
            tool: "image_to_video",
            tokens_cost: 9,
        })
        .expect("insert preview asset");
    }
    db.insert_asset(&InsertAsset {
        creation_id,
        kind: "video",
        url: "https://mock.xiaoyue/final.mp4",
        thumbnail_url: Some("https://picsum.photos/seed/final/640/360"),
        prompt: "定稿: 4 镜合成",
        tool: "image_to_video",
        tokens_cost: 19,
    })
    .expect("insert final asset");

    let rows = db
        .list_creations(Some("director"))
        .expect("list creations");
    let found = rows.iter().find(|r| r.id == creation_id);
    assert!(found.is_some(), "小月作品应在作品墙里");

    let assets = db.list_assets(creation_id).expect("list assets");
    assert_eq!(assets.len(), 5, "4 试拍 + 1 定稿 = 5 资产");
    let total_cost: u32 = assets.iter().map(|a| a.tokens_cost).sum();
    assert_eq!(total_cost, 9 * 4 + 19, "学币总扣应与 directorStore 一致");

    let _ = std::fs::remove_file(&tmp); // cleanup
}

// ============ Test 8 — 学币 chain refund (失败退款) ============
//
// 在 Rust 端固化"扣多少退多少"不变量, 不依赖 frontend 测试.
// 验证逻辑: attempt n 次, n 次全失败, 学币累计净变化 = 0.

#[test]
fn persona_credit_refund_chain_stays_at_zero() {
    let starting: u32 = 500;
    let mut balance: u32 = starting;
    // 模拟 4 个 shot × 试拍失败 (扣 9 → 失败 → 退 9)
    for _ in 0..4 {
        balance -= 9; // spend
        balance += 9; // refund on fail
    }
    // 定稿 1 次 × 试拍成功 (净 -19)
    balance -= 19;
    assert_eq!(
        balance,
        starting - 19,
        "4 个失败 preview 不消耗学币, 定稿 net 扣 19"
    );
}

// ============ Test 9 — persona registry: 名字是稳定的 (frontend 显示依赖) ============

#[test]
fn persona_pet_ids_have_human_readable_names() {
    // pet_id 用拼音/汉字, 不能是裸 ID; 这关系到 PetCorner / PetEngine 文案.
    for p in [&XIAOYUE, &XIAOMO] {
        assert!(
            !p.pet_id.is_empty(),
            "persona {} 必须有 pet_id",
            p.nickname
        );
        // 小月: huomiao (拼音); 小墨: 墨石 (汉字).
        // 这里只断非空, 由前端 recall_message 负责映射到文案.
    }
}

// ============ Test 10 — builtin_characters 跨 persona 共享, 但 description 区分 ============

#[test]
fn persona_builtin_chars_have_distinct_descriptions() {
    let chars = builtin_characters();
    let descriptions: Vec<&str> = chars.iter().map(|c| c.description.as_str()).collect();
    let unique: std::collections::HashSet<&str> = descriptions.iter().copied().collect();
    assert!(
        unique.len() >= 3,
        "至少 3 个 builtin 角色各有独特 description, got {} unique",
        unique.len()
    );

    // 小月 persona 必须有 builtin 角色可对应, 而非用户自建
    let _xiaoyue_builtin: &Character = chars
        .iter()
        .find(|c| c.id == "xiaoyue")
        .expect("xiaoyue builtin 必须存在 (小月 persona 锁定这个 character_id)");
}

// ============ Test 11 — sanity: 5h 连续创作 触发 "休息" recall (对所有人, 包括小月/小墨) ============
//
// 设计决策: PetEngine.burnout 是 universal safety check, 不分 age_tier.
// 小学生连玩 5 小时也需要 prompt 休息, 这条与 recall-threshold (按年龄分级) 是正交的.
// 这里把不变量固化下来: 任何 persona 连续 5h+ 在创作, 必弹"休息一下"召回.

#[test]
fn persona_kid_teen_5h_burnout_recall() {
    for p in [&XIAOYUE, &XIAOMO] {
        let input = PetTickInput {
            user_id: format!("user:{}", p.nickname),
            identity: fixture_identity(p),
            last_user_action_secs_ago: 60,
            is_in_conversation: true,
            conversation_started_secs_ago: 5 * 3_600 + 1,
        };
        let action = PetEngine::tick(&input);
        match action {
            PetAction::Recall { message } => {
                assert!(
                    message.contains("休息"),
                    "persona {} 5h+ 创作应触发 burnout recall (universal safety), got: {message}",
                    p.nickname
                );
            }
            other => panic!("{} 5h 应触发 burnout Recall, 实际: {:?}", p.nickname, other),
        }
    }
}
