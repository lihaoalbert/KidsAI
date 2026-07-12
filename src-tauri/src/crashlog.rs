// W4.5 D1: 桌面崩溃日志 — 把 eprintln! 输出和 panic 写入 app_data_dir/logs/
// 依赖: 仅 std (无 log crate, 避免引入额外 deps).
// 轮转: 每天一个新文件 kidsai-YYYY-MM-DD.log. 启动时打开今日文件 append.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::panic;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// 全局日志文件句柄. 启动时 init() 一次, 此后所有 event() 调用写入它.
static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);
static LOG_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// 启动时调一次. 在 app setup() 开头调, 早于其他 init, 这样后续 eprintln 也能转过来.
pub fn init(app_data_dir: &Path) {
    let logs_dir = app_data_dir.join("logs");
    if let Err(e) = std::fs::create_dir_all(&logs_dir) {
        // 不能创建日志目录也别 panic — 只是没有日志可写
        eprintln!("[crashlog] failed to create {:?}: {}", logs_dir, e);
        return;
    }

    // 锁 LOG_DIR 持有路径 (供 get_log_dir 用)
    if let Ok(mut g) = LOG_DIR.lock() {
        *g = Some(logs_dir.clone());
    }

    if let Ok(file) = open_today(&logs_dir) {
        if let Ok(mut g) = LOG_FILE.lock() {
            *g = Some(file);
        }
    }

    // 安装 panic hook: 崩溃时写 panic message + location 到日志 + 终端
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let loc = panic_info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let payload = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic>".to_string()
        };
        event("PANIC", &format!("at {}: {}", loc, payload));
        // 仍调默认 hook 让终端也打 (开发期排障)
        default_hook(panic_info);
    }));

    event(
        "STARTUP",
        &format!("KidsAI Studio v{}", env!("CARGO_PKG_VERSION")),
    );
}

/// 写一条日志. 同时写到文件 (如果 init 过) 和 stderr (开发期可见).
pub fn event(tag: &str, msg: &str) {
    let line = format!(
        "{} [{}] {}\n",
        chrono_like_now(),
        tag,
        msg.trim_end()
    );
    if let Ok(mut g) = LOG_FILE.lock() {
        if let Some(f) = g.as_mut() {
            let _ = f.write_all(line.as_bytes());
            let _ = f.flush();
        }
    }
    // stderr 同步输出 (开发期 / 没初始化时也可见)
    let _ = std::io::stderr().write_all(line.as_bytes());
}

/// 返回日志目录 (给前端"打开日志文件夹"按钮用). None = 未初始化.
pub fn log_dir() -> Option<PathBuf> {
    LOG_DIR.lock().ok().and_then(|g| g.clone())
}

/// 替换 eprintln! 的结构化调用.
#[macro_export]
macro_rules! elog {
    ($($arg:tt)*) => {{
        $crate::crashlog::event("info", &format!($($arg)*));
    }};
}

/// 简易时间戳 (避免引入 chrono). 格式: "YYYY-MM-DD HH:MM:SS".
fn chrono_like_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let s_of_day = (secs % 86_400) as i64;
    let hh = s_of_day / 3600;
    let mm = (s_of_day % 3600) / 60;
    let ss = s_of_day % 60;
    // 1970-01-01 是周四. 用 Howard Hinnant 算法 (civil_from_days) 算日期.
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        y, m, d, hh, mm, ss
    )
}

/// Howard Hinnant civil_from_days: 把 days since 1970-01-01 转成 (Y, M, D).
fn civil_from_days(days_since_1970: i64) -> (i64, u32, u32) {
    let z = days_since_1970 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

fn open_today(logs_dir: &Path) -> std::io::Result<File> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let (y, m, d) = civil_from_days(days);
    let path = logs_dir.join(format!("kidsai-{:04}-{:02}-{:02}.log", y, m, d));
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_from_days_zero_is_1970_01_01() {
        let (y, m, d) = civil_from_days(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn civil_from_days_one_day() {
        let (y, m, d) = civil_from_days(1);
        assert_eq!((y, m, d), (1970, 1, 2));
    }

    #[test]
    fn civil_from_days_year_boundary() {
        // 2024-01-01 是 1970-01-01 起的第 19723 天
        let (y, m, d) = civil_from_days(19723);
        assert_eq!((y, m, d), (2024, 1, 1));
    }
}