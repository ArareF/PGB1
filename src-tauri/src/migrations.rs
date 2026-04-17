//! 安装目录迁移：清理 v2.8.10 之前的中文 productName 旧安装目录
//!
//! v2.8.10 起 productName 由 "PG素材管理系统" → "PGB1"（规避 GitHub Release
//! 中文文件名被吞）。副作用：老用户自动升级后新版装到 Programs\PGB1\，
//! 旧版 Programs\PG素材管理系统\ 成为孤儿，桌面/开始菜单老快捷方式仍指向
//! 旧 exe → 用户重启后进入 2.8.9。
//!
//! 本模块在新版首次启动时静默清理旧安装目录与快捷方式。
//! 爆炸半径：仅固定字面量路径，不 glob、不递归搜索、不碰 AppData\Roaming。

use std::path::{Path, PathBuf};

const LEGACY_PRODUCT_NAME: &str = "PG素材管理系统";

/// 清理旧 productName 的安装残留。
///
/// 静默模式：探测不到旧目录 → 零操作返回；失败只记日志不中断启动。
pub fn migrate_legacy_install() {
    if let Err(e) = run_migration() {
        log::error!("[migrate] 清理旧安装目录失败: {}", e);
    }
}

fn run_migration() -> Result<(), String> {
    let local_app_data = std::env::var("LOCALAPPDATA")
        .map_err(|e| format!("读取 LOCALAPPDATA 失败: {}", e))?;

    let legacy_dir = PathBuf::from(&local_app_data)
        .join("Programs")
        .join(LEGACY_PRODUCT_NAME);

    if !legacy_dir.is_dir() {
        return Ok(());
    }

    // 安全校验：目录里至少有一个 .exe 文件才认定为 NSIS 安装目录
    // 防御用户自己建同名目录放别的东西（极端 case，但破坏性操作必须守底线）
    if !dir_contains_exe(&legacy_dir) {
        log::warn!(
            "[migrate] {} 存在但无 .exe，非旧安装目录，跳过清理",
            legacy_dir.display()
        );
        return Ok(());
    }

    log::info!("[migrate] 开始清理旧安装目录: {}", legacy_dir.display());
    std::fs::remove_dir_all(&legacy_dir)
        .map_err(|e| format!("删除旧目录 {} 失败: {}", legacy_dir.display(), e))?;

    cleanup_desktop_shortcut();
    cleanup_start_menu_folder();

    log::info!("[migrate] 旧安装目录清理完成");
    Ok(())
}

fn dir_contains_exe(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries
        .filter_map(|e| e.ok())
        .any(|e| e.path().extension().and_then(|s| s.to_str()) == Some("exe"))
}

fn cleanup_desktop_shortcut() {
    let Ok(userprofile) = std::env::var("USERPROFILE") else {
        return;
    };
    let desktop_lnk = PathBuf::from(userprofile)
        .join("Desktop")
        .join(format!("{}.lnk", LEGACY_PRODUCT_NAME));
    if desktop_lnk.exists() {
        if let Err(e) = std::fs::remove_file(&desktop_lnk) {
            log::warn!("[migrate] 删除桌面快捷方式失败: {}", e);
        }
    }
}

fn cleanup_start_menu_folder() {
    let Ok(appdata) = std::env::var("APPDATA") else {
        return;
    };
    let start_menu_dir = PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join(LEGACY_PRODUCT_NAME);
    if start_menu_dir.is_dir() {
        if let Err(e) = std::fs::remove_dir_all(&start_menu_dir) {
            log::warn!("[migrate] 删除开始菜单目录失败: {}", e);
        }
    }
}
