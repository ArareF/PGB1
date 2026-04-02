use crate::models::AppSettings;
use std::fs;
use std::path::Path;
use tauri::{AppHandle, Manager, Runtime};

/// 加载应用设置，若不存在则探测并创建
#[tauri::command]
pub fn load_settings<R: Runtime>(app_handle: AppHandle<R>) -> Result<AppSettings, String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录: {}", e))?;
    let config_path = config_dir.join("app_settings.json");

    if config_path.exists() {
        let content = fs::read_to_string(&config_path).map_err(|e| format!("读取设置失败: {}", e))?;
        let mut settings: AppSettings = serde_json::from_str(&content).map_err(|e| format!("解析设置失败: {}", e))?;

        // 迁移修复：旧版本 GUI→CLI 互推可能产生 \bin\bin\ 双层路径
        let cli = &settings.workflow.texture_packer_cli_path;
        if cli.contains(r"\bin\bin\") || cli.contains("/bin/bin/") {
            settings.workflow.texture_packer_cli_path =
                cli.replace(r"\bin\bin\", r"\bin\").replace("/bin/bin/", "/bin/");
            log::warn!("迁移修复：TP CLI 路径 \\bin\\bin\\ → \\bin\\");
            let _ = save_settings_to_file(&config_dir, &settings);
        }

        return Ok(settings);
    }

    // 首次运行：创建默认配置（工具路径探测交给前端 OnboardingDialog）
    let settings = AppSettings::default();
    save_settings(app_handle, settings.clone())?;

    Ok(settings)
}

/// 纯写文件，不触发 autolaunch 等副作用（供迁移修复使用）
fn save_settings_to_file(config_dir: &Path, settings: &AppSettings) -> Result<(), String> {
    if !config_dir.exists() {
        fs::create_dir_all(config_dir).map_err(|e| format!("创建配置目录失败: {}", e))?;
    }
    let config_path = config_dir.join("app_settings.json");
    let json = serde_json::to_string_pretty(settings).map_err(|e| format!("序列化设置失败: {}", e))?;
    fs::write(&config_path, json).map_err(|e| format!("写入设置文件失败: {}", e))?;
    Ok(())
}

/// 保存应用设置
#[tauri::command]
pub fn save_settings<R: Runtime>(app_handle: AppHandle<R>, settings: AppSettings) -> Result<(), String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录: {}", e))?;

    save_settings_to_file(&config_dir, &settings)?;

    // 同步开机自启注册表
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app_handle.autolaunch();
    if settings.general.auto_start {
        autolaunch.enable().map_err(|e| format!("设置开机自启失败: {}", e))?;
    } else if autolaunch.is_enabled().unwrap_or(false) {
        autolaunch.disable().map_err(|e| format!("取消开机自启失败: {}", e))?;
    }

    Ok(())
}
