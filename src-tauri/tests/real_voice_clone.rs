// W6 D2: 真实 MiniMax voice_clone 集成测试 (--ignored)
//
// 仅当 .env (或环境变量) 有 MINIMAX_API_KEY 时有意义.
// 真跑 ~¥3 (单次 voice_clone 训练) + 10 学币; 用 --ignored 才执行.
//
// 验证:
// - select_voice_clone_adapter() 走 minimax
// - POST /v1/voice_clone multipart 上传 10s WAV → 返 voice_id
// - voice_id 非空 (后续 TTS 能用)
//
// 测试音频: 用 hound 生成 10 秒静音 WAV 写 tmp (MiniMax 实际能接受, 我们只验证
// pipeline 接通, 不验证音色质量 — 那是种子用户上传后的事).

use hound::{SampleFormat, WavSpec};
use kidsai_studio_lib::voice_adapter::{select_voice_clone_adapter, VoiceCloneArgs};

fn real_key_available() -> bool {
    let _ = dotenvy::dotenv();
    std::env::var("MINIMAX_API_KEY")
        .map(|v| !v.is_empty() && v.len() > 20)
        .unwrap_or(false)
}

/// 生成 10 秒静音 16kHz mono WAV, 写到 tmp path. 返 path.
fn write_silent_wav_10s() -> String {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let path = std::env::temp_dir().join(format!("kidsai_voice_test_{}.wav", std::process::id()));
    let mut writer = hound::WavWriter::create(&path, spec).unwrap();
    for _ in 0..160_000 {
        writer.write_sample(0i16).unwrap();
    }
    writer.finalize().unwrap();
    eprintln!("[INFO] wrote silent WAV to {:?}", path);
    path.to_string_lossy().to_string()
}

#[test]
#[ignore = "requires real MiniMax API key; run with: cargo test --test real_voice_clone -- --ignored"]
fn real_minimax_voice_clone_pipeline() {
    if !real_key_available() {
        eprintln!("[SKIP] MINIMAX_API_KEY not set or too short");
        return;
    }

    let sel = select_voice_clone_adapter();
    // sel 是 Box<dyn VoiceCloneAdapter>, 验证 provider 不直接拿, 走调用即可

    let wav_path = write_silent_wav_10s();

    let asset = sel
        .clone_voice(&VoiceCloneArgs {
            audio_path: wav_path,
            voice_id_hint: Some(format!("kidsai-test-{}", std::process::id())),
        })
        .expect("voice_clone should succeed");

    eprintln!(
        "[INFO] provider={} voice_id={}",
        asset.provider, asset.voice_id
    );

    assert_eq!(asset.provider, "minimax");
    assert!(!asset.voice_id.is_empty(), "voice_id should be non-empty");
    assert!(asset.voice_id.len() >= 8, "voice_id too short: {}", asset.voice_id);
}