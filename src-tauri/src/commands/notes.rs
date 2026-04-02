use super::helpers::read_notes_file;
use std::fs;
use std::path::Path;

/// 获取目录下所有笔记
#[tauri::command]
pub fn get_notes(dir_path: String) -> Result<std::collections::HashMap<String, String>, String> {
    let dir = Path::new(&dir_path);
    Ok(read_notes_file(dir))
}

/// 设置/删除单条笔记（note 为 None 或空字符串时删除该 key，Map 为空时删除文件）
#[tauri::command]
pub fn set_note(dir_path: String, key: String, note: Option<String>) -> Result<(), String> {
    let dir = Path::new(&dir_path);
    let notes_path = dir.join(".pgb1_notes.json");
    let mut map = read_notes_file(dir);

    match note {
        Some(text) if !text.is_empty() => { map.insert(key, text); }
        _ => { map.remove(&key); }
    }

    if map.is_empty() {
        // Map 为空时删除文件（避免留空文件）
        if notes_path.exists() {
            let _ = fs::remove_file(&notes_path);
        }
    } else {
        let json = serde_json::to_string_pretty(&map)
            .map_err(|e| format!("序列化笔记失败: {}", e))?;
        fs::write(&notes_path, json)
            .map_err(|e| format!("写入笔记失败: {}", e))?;
    }

    Ok(())
}
