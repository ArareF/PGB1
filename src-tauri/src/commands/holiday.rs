//! 状态栏节假日 / IP 地理位置外部 API 代理
//!
//! 三个命令统一从 Rust 端发起 HTTPS 请求，前端不再直连：
//! - `fetch_ip_country`      → ipapi.co（用户 IP 定位，2 字母国家代码）
//! - `fetch_cn_holiday_type` → timor.tech（中国节假日类型：0 工作日 / 1 假日 / 2 补休工作）
//! - `fetch_nager_holidays`  → date.nager.at（国际节假日日期列表）
//!
//! 动机：
//! 1. 避免用户 IP 直接泄漏给三方服务（Y-13）
//! 2. 前端 CSP `connect-src` 可以从 `'self' https:` 收敛到具体域名白名单（Y-14）

use serde::Deserialize;

const HTTP_TIMEOUT_SECS: u64 = 10;

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("构建 HTTP 客户端失败: {}", e))
}

// ─── ipapi.co ───────────────────────────────────────────────

/// 通过 ipapi.co 检测用户 IP 所在国家，返回 2 字母国家代码（大写）
///
/// 失败（网络异常 / 非法响应）返回 Err，前端按原逻辑降级为默认地区。
#[tauri::command]
pub async fn fetch_ip_country() -> Result<String, String> {
    let client = build_client()?;
    let res = client
        .get("https://ipapi.co/country/")
        .send()
        .await
        .map_err(|e| format!("请求 ipapi.co 失败: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("ipapi.co HTTP {}", res.status()));
    }

    let text = res
        .text()
        .await
        .map_err(|e| format!("读取 ipapi.co 响应失败: {}", e))?
        .trim()
        .to_uppercase();

    // 只接受标准 2 字母国家代码，防止收到广告页 / 错误文本
    if text.len() == 2 && text.chars().all(|c| c.is_ascii_uppercase()) {
        Ok(text)
    } else {
        Err(format!("ipapi.co 返回非法国家代码: {:?}", text))
    }
}

// ─── timor.tech（中国节假日）─────────────────────────────────

#[derive(Deserialize)]
struct TimorResponse {
    #[serde(rename = "type")]
    type_obj: Option<TimorType>,
}

#[derive(Deserialize)]
struct TimorType {
    #[serde(rename = "type")]
    type_num: Option<i32>,
}

/// 查询 timor.tech 的单日 CN 节假日类型
///
/// 返回值含义：
/// - `Some(0)` → 工作日
/// - `Some(1)` → 法定节假日
/// - `Some(2)` → 调休工作日
/// - `None`    → API 响应缺字段（前端对齐原来的 `null`）
#[tauri::command]
pub async fn fetch_cn_holiday_type(
    year: i32,
    month: u32,
    day: u32,
) -> Result<Option<i32>, String> {
    let url = format!(
        "https://timor.tech/api/holiday/info/{}-{}-{}",
        year, month, day
    );
    let client = build_client()?;
    let res = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("请求 timor.tech 失败: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("timor.tech HTTP {}", res.status()));
    }

    let data: TimorResponse = res
        .json()
        .await
        .map_err(|e| format!("解析 timor.tech 响应失败: {}", e))?;

    Ok(data.type_obj.and_then(|t| t.type_num))
}

// ─── date.nager.at（国际节假日）──────────────────────────────

#[derive(Deserialize)]
struct NagerHoliday {
    date: String,
}

/// 查询 date.nager.at 的某国某年所有公共节假日
///
/// 返回 "YYYY-MM-DD" 格式的日期数组（可能为空）。
/// HTTP 非 200 返回 Err 让前端走降级路径（空 Set）。
#[tauri::command]
pub async fn fetch_nager_holidays(
    year: i32,
    country_code: String,
) -> Result<Vec<String>, String> {
    let code = country_code.trim().to_uppercase();
    if code.len() != 2 || !code.chars().all(|c| c.is_ascii_uppercase()) {
        return Err(format!("非法国家代码: {:?}", country_code));
    }

    let url = format!(
        "https://date.nager.at/api/v3/PublicHolidays/{}/{}",
        year, code
    );
    let client = build_client()?;
    let res = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("请求 date.nager.at 失败: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("date.nager.at HTTP {}", res.status()));
    }

    let data: Vec<NagerHoliday> = res
        .json()
        .await
        .map_err(|e| format!("解析 date.nager.at 响应失败: {}", e))?;

    Ok(data.into_iter().map(|h| h.date).collect())
}
