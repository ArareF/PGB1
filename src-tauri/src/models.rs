use serde::{Deserialize, Serialize};

/// 项目配置文件（.pgb1_project.json）的结构
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectConfig {
    pub project_name: String,
    pub created_at: String,
    #[serde(default)]
    pub imported: bool,
    pub deadline: Option<String>,
    pub enabled_tasks: Vec<String>,
    #[serde(default)]
    pub archived_tasks: Vec<String>,
    /// 已完成的子任务列表（"TaskName/ChildName" 格式）
    #[serde(default)]
    pub completed_subtasks: Vec<String>,
    /// 已弹过上传提醒的任务名列表（小写）
    #[serde(default)]
    pub upload_prompted_tasks: Vec<String>,
    /// 默认打开的 AE 工程文件名（仅文件名，不含路径）
    #[serde(default)]
    pub default_ae_file: Option<String>,
    /// 项目优先度（"high" / "medium" / "low"），null 表示无
    #[serde(default)]
    pub priority: Option<String>,
    /// 任务优先度 Map（task_name_lower → priority）
    #[serde(default)]
    pub task_priorities: std::collections::HashMap<String, String>,
}

/// 返回给前端的项目信息
#[derive(Debug, Serialize, Clone)]
pub struct ProjectInfo {
    /// 项目名称（目录名），如 "217_RedDevil"
    pub name: String,
    /// 项目完整路径
    pub path: String,
    /// 截止日期，如 "2026-03-15" 或 null
    pub deadline: Option<String>,
    /// 任务名称列表
    pub tasks: Vec<String>,
    /// 任务总数
    pub task_count: usize,
    /// 启用的任务列表（小写名称）
    pub enabled_tasks: Vec<String>,
    /// 已完成的子任务列表
    pub completed_subtasks: Vec<String>,
    /// 已弹过上传提醒的任务名列表
    pub upload_prompted_tasks: Vec<String>,
    /// 已完成的无子任务父任务列表（全素材已上传到 nextcloud）
    pub completed_tasks: Vec<String>,
    /// 默认打开的 AE 工程文件名（仅文件名）
    pub default_ae_file: Option<String>,
    /// AppIcon 文件的绝对路径（来自 01_Preproduction/，名含 appicon，优先 PNG 其次 PSD/PSB）
    pub app_icon: Option<String>,
    /// 项目优先度（来自 ProjectConfig.priority）
    pub priority: Option<String>,
    /// 卡片批注（来自 .pgb1_notes.json，key = card:{项目名_lower}）
    pub note: Option<String>,
}

/// 返回给前端的任务信息
#[derive(Debug, Serialize, Clone)]
pub struct TaskInfo {
    /// 任务名称（目录名），如 "Ambient"
    pub name: String,
    /// 任务完整路径（Export 下的目录）
    pub path: String,
    /// 文件夹总大小（字节）
    pub size_bytes: u64,
    /// 是否有子任务（Prototype 特例）
    pub has_subtasks: bool,
    /// 素材总数（00_original 中）
    pub material_total: u32,
    /// 已上传素材数（存在于 nextcloud 中）
    pub material_uploaded: u32,
    /// 预览视频总数（03_preview 中）
    pub video_total: u32,
    /// 已上传预览视频数（存在于 nextcloud/preview/ 中）
    pub video_uploaded: u32,
    /// 任务优先度（来自项目 .pgb1_project.json 的 task_priorities）
    pub priority: Option<String>,
    /// 卡片批注（来自 .pgb1_notes.json，key = card:{任务名_lower}）
    pub note: Option<String>,
}

/// 通用文件/目录条目（普通卡片用）
#[derive(Debug, Serialize, Clone)]
pub struct FileEntry {
    /// 文件/目录名
    pub name: String,
    /// 完整路径
    pub path: String,
    /// 是否为目录
    pub is_dir: bool,
    /// 文件大小（字节），目录为 0
    pub size_bytes: u64,
    /// 文件扩展名（小写，无点号），如 "png"、"mp4"
    pub extension: String,
}

/// 素材文件类型
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MaterialType {
    /// 静帧图片
    Image,
    /// 序列帧动画（文件夹包含多帧）
    Sequence,
    /// 视频文件
    Video,
    /// 其他文件
    Other,
}

/// 素材进度状态
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MaterialProgress {
    /// 未开始（通过 serde 序列化为 "none" 供前端使用）
    #[allow(dead_code)]
    None,
    /// 原始文件（仅在 00_original）
    Original,
    /// 已缩放（存在于 01_scale，仅静帧）
    Scaled,
    /// 已完成（存在于 02_done）
    Done,
    /// 已上传（存在于 nextcloud）
    Uploaded,
    /// 链条断裂（中间环节缺失，需要用户检查）
    Broken,
}

/// 返回给前端的素材信息
#[derive(Debug, Serialize, Clone)]
pub struct MaterialInfo {
    /// 素材基础名（去掉扩展名和 _01 后缀）
    pub name: String,
    /// 完整文件名或目录名（在 00_original 中）
    pub file_name: String,
    /// 00_original 中的完整路径
    pub path: String,
    /// 素材类型
    pub material_type: MaterialType,
    /// 进度状态
    pub progress: MaterialProgress,
    /// 文件大小（字节），序列帧为整个目录大小
    pub size_bytes: u64,
    /// 序列帧帧数（非序列帧为 0）
    pub frame_count: u32,
    /// 文件扩展名（小写）
    pub extension: String,
    /// 预览图路径（静帧=文件本身，序列帧=首帧）
    pub preview_path: Option<String>,
    /// 已缩放的比例列表（静帧：扫描 01_scale/[XX]/ 得出；序列帧/其他：空）
    pub scales: Vec<u32>,
    /// 序列帧帧率（从 02_done/[an-XX-YY]/ 目录名解析；转换前为 None）
    pub fps: Option<u32>,
}

/// 素材版本信息（侧边栏"其他版本"用）
#[derive(Debug, Serialize, Clone)]
pub struct MaterialVersion {
    /// 阶段名称（"00_original", "01_scale", "02_done", "nextcloud"）
    pub stage: String,
    /// 阶段中文标签（"原始", "已缩放", "已完成", "已上传"）
    pub stage_label: String,
    /// 缩放比例（如 "100"、"70"、"50"），原始阶段为空
    pub scale: String,
    /// 文件完整路径
    pub file_path: String,
    /// 所在目录路径（用于"打开文件夹"）
    pub folder_path: String,
    /// 文件扩展名
    pub extension: String,
    /// 文件大小（字节）
    pub size_bytes: u64,
}

/// 拖拽请求中的素材信息（前端传入）
#[derive(Debug, Deserialize, Clone)]
pub struct DragMaterialRequest {
    pub name: String,
    pub material_type: String,
}

/// 复制到 nextcloud 请求中的素材信息
#[derive(Debug, Deserialize, Clone)]
pub struct CopyMaterialRequest {
    pub name: String,
    pub material_type: String,
}

/// 复制到 nextcloud 的结果
#[derive(Debug, Serialize, Clone)]
pub struct CopyResult {
    pub copied_count: u32,
    pub errors: Vec<String>,
}

/// 通用文件导入结果
#[derive(Debug, Serialize, Clone)]
pub struct ImportResult {
    pub imported_count: u32,
    pub skipped_count: u32,
    pub errors: Vec<String>,
}

// ─── 任务管理系统 ─────────────────────────────────────────────

/// 全局任务清单配置（.pgb1_global_tasks.json）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GlobalTaskConfig {
    pub tasks: Vec<GlobalTask>,
}

/// 全局任务（可含子任务）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GlobalTask {
    pub name: String,
    #[serde(default)]
    pub children: Vec<GlobalTaskChild>,
}

/// 全局任务的子任务
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GlobalTaskChild {
    pub name: String,
}

/// apply_task_changes 的执行结果
#[derive(Debug, Serialize, Clone)]
pub struct ApplyTaskResult {
    pub created: Vec<String>,
    pub archived: Vec<String>,
    pub errors: Vec<String>,
}

// ─── 日报打卡系统 ─────────────────────────────────────────────


fn default_attendance_mode() -> String {
    "auto".to_string()
}

fn default_true() -> bool {
    true
}

/// 日报打卡配置（持久化到 attendance_config.json）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AttendanceConfig {
    /// 打卡模式：“off” 关闭 / “auto” 自动打卡 / “record_only” 仅记录时间
    #[serde(default = "default_attendance_mode")]
    pub mode: String,
    pub attendance: AttendanceSettings,
    pub daily_report: DailyReportSettings,
    /// 账号（明文存储，密码不在这里）
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AttendanceSettings {
    /// 出勤提醒时间 "HH:mm"
    pub clock_in_time: String,
    /// 退勤提醒时间 "HH:mm"
    pub clock_out_time: String,
    /// 打卡网站 URL
    pub url: String,
    /// 午休开始时间 "HH:mm"（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lunch_start_time: Option<String>,
    /// 午休结束时间 "HH:mm"（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lunch_end_time: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DailyReportSettings {
    /// 日报提醒开关（默认开启）
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 日报提醒时间 "HH:mm"
    pub time: String,
    /// 日报网站 URL
    pub url: String,
}

impl Default for AttendanceConfig {
    fn default() -> Self {
        Self {
            mode: "auto".to_string(),
            attendance: AttendanceSettings {
                clock_in_time: "09:50".to_string(),
                clock_out_time: "19:00".to_string(),
                url: String::new(),
                lunch_start_time: None,
                lunch_end_time: None,
            },
            daily_report: DailyReportSettings {
                enabled: true,
                time: "18:30".to_string(),
                url: String::new(),
            },
            username: String::new(),
        }
    }
}

/// 本地打卡记录（仅用于判断"今天是否已打卡"）
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AttendanceRecord {
    /// 最后出勤打卡日期 "YYYY-MM-DD"
    pub last_clock_in: Option<String>,
    /// 最后退勤打卡日期 "YYYY-MM-DD"
    pub last_clock_out: Option<String>,
    /// 用户主动关闭出勤提醒的日期 "YYYY-MM-DD"（每天只弹一次用）
    pub dismissed_clock_in_date: Option<String>,
    /// 实际出勤打卡时间 "HH:mm"（打卡成功时写入，状态栏用真实时间算工时）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_clock_in_time: Option<String>,
    /// 实际退勤打卡时间 "HH:mm"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_clock_out_time: Option<String>,
}

/// 归档版本信息（时光机用）
#[derive(Debug, Serialize, Clone)]
pub struct ArchivedVersion {
    /// 任务名称（目录名），如 "Ambient"
    pub task_name: String,
    /// 时间戳原始值，如 "2026-02-13_14-30"
    pub timestamp: String,
    /// 前端展示用时间，如 "2026-02-13 14:30"
    pub display_time: String,
    /// 归档版本完整路径
    pub path: String,
}

// ─── 规范化功能 ─────────────────────────────────────────────

/// 规范化操作类型
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NormalizeActionType {
    /// 重命名文件（用于静帧去 _01）
    Rename,
    /// 创建文件夹并移动（用于序列帧）
    MoveToFolder,
}

/// 规范化单项操作预览
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NormalizePreviewItem {
    pub original_path: String,
    pub original_name: String,
    pub target_name: String,
    pub action_type: NormalizeActionType,
    pub is_sequence: bool,
}

/// 缩放请求中的素材信息
#[derive(Debug, Deserialize, Clone)]
pub struct ScaleRequest {
    pub original_path: String,
    pub target_dir: String,
    pub scale_percent: u32,
    pub base_name: String,
}

// ─── 转换功能 ─────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct ConversionSequenceRequest {
    pub name: String,
    pub fps: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StartConversionRequest {
    pub task_path: String,
    /// 文件名 -> 比例（前端可能传 0 让后端自己识别）
    pub images: HashMap<String, u32>,
    pub sequences: Vec<ConversionSequenceRequest>,
    pub imagine_path: String,
    pub texture_packer_cli_path: String,
    pub texture_packer_gui_path: String,
    pub tp_scale: f64,
    pub tp_webp_quality: u32,
}

// ─── 全局应用设置 ─────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub workflow: WorkflowSettings,
    pub translation: TranslationSettings,
    pub general: GeneralSettings,
    #[serde(default)]
    pub preview: PreviewSettings,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSettings {
    pub imagine_path: String,
    pub texture_packer_cli_path: String,
    pub texture_packer_gui_path: String,
    #[serde(default = "default_tp_scale")]
    pub tp_scale: f64,
    #[serde(default = "default_tp_webp_quality")]
    pub tp_webp_quality: u32,
}

fn default_tp_scale() -> f64 { 0.5 }
fn default_tp_webp_quality() -> u32 { 80 }

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TranslationSettings {
    pub api_key: String,
    pub model: String,
    pub shortcut: String,
    pub use_calculator_key: bool,
    #[serde(default = "default_lang_a")]
    pub lang_a: String,
    #[serde(default = "default_lang_b")]
    pub lang_b: String,
}

fn default_lang_a() -> String { "zh-CN".to_string() }
fn default_lang_b() -> String { "en".to_string() }

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSettings {
    #[serde(default = "default_preview_fps")]
    pub default_fps: u32,
    #[serde(default)]
    pub background_transparent: bool,
}

fn default_preview_fps() -> u32 { 24 }

impl Default for PreviewSettings {
    fn default() -> Self {
        Self { default_fps: 24, background_transparent: false }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettings {
    pub project_root_dir: String,
    /// UI 缩放比例（0.0 = 自动跟随窗口，> 0 = 手动固定值）
    #[serde(default)]
    pub ui_scale: f32,
    /// 开机自启动
    #[serde(default)]
    pub auto_start: bool,
    /// 界面语言（"zh-CN" | "en"）
    #[serde(default = "default_language")]
    pub language: String,
    /// 是否已完成首次引导
    #[serde(default)]
    pub onboarded: bool,
}

fn default_language() -> String { "zh-CN".to_string() }

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            workflow: WorkflowSettings {
                imagine_path: String::new(),
                texture_packer_cli_path: String::new(),
                texture_packer_gui_path: String::new(),
                tp_scale: default_tp_scale(),
                tp_webp_quality: default_tp_webp_quality(),
            },
            translation: TranslationSettings {
                api_key: String::new(),
                model: "gemini-2.5-flash".to_string(),
                shortcut: "Ctrl+Shift+T".to_string(),
                use_calculator_key: false,
                lang_a: "zh-CN".to_string(),
                lang_b: "en".to_string(),
            },
            general: GeneralSettings {
                project_root_dir: String::new(),
                ui_scale: 1.0,
                auto_start: false,
                language: "zh-CN".to_string(),
                onboarded: false,
            },
            preview: PreviewSettings::default(),
        }
    }
}

// ─── 快捷方式栏 ─────────────────────────────────────────────

/// 快捷方式类型
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutType {
    /// 应用程序（.exe）
    App,
    /// 文件夹
    Folder,
    /// 网页 URL
    Web,
}

/// 单个快捷方式
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Shortcut {
    /// 唯一 ID（UUID）
    pub id: String,
    /// 类型
    pub shortcut_type: ShortcutType,
    /// 用户自定义名称
    pub name: String,
    /// 路径（应用/文件夹）或 URL（网页）
    pub path: String,
    /// 图标缓存路径（相对于 app_config_dir/shortcut_icons/），null 表示使用默认图标
    pub icon_cache: Option<String>,
    /// 排序序号
    pub order: u32,
}

/// 快捷方式配置文件结构
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ShortcutsConfig {
    pub shortcuts: Vec<Shortcut>,
}

/// Windows 系统应用（从 .lnk 解析出来，供添加弹窗列表用）
#[derive(Debug, Serialize, Clone)]
pub struct AppShortcut {
    /// 显示名称（lnk 文件名去掉 .lnk 后缀）
    pub name: String,
    /// 目标 exe 路径
    pub target_path: String,
}

/// 预览视频文件条目（含上传状态）
#[derive(Debug, Serialize, Clone)]
pub struct PreviewVideoEntry {
    pub name: String,
    pub path: String,
    pub extension: String,
    pub size_bytes: u64,
    /// "uploaded" | "outdated" | "none"
    pub upload_status: String,
}

use std::collections::HashMap;

// ─── 贴图板 ──────────────────────────────────────────────

/// 单条标注
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PinAnnotation {
    #[serde(rename = "type")]
    pub annotation_type: String,
    pub color: String,
    #[serde(default)]
    pub stroke_width: f64,
    #[serde(default)]
    pub points: Vec<[f64; 2]>,
    #[serde(default)]
    pub start: Option<[f64; 2]>,
    #[serde(default)]
    pub end: Option<[f64; 2]>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub position: Option<[f64; 2]>,
    #[serde(default)]
    pub font_size: Option<f64>,
}

/// 单张贴图
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PinInfo {
    pub id: String,
    pub image: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub annotations: Vec<PinAnnotation>,
    #[serde(default)]
    pub z_index: u32,
    #[serde(default)]
    pub created_at: String,
}

/// 画布视口状态
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PinboardViewport {
    #[serde(default)]
    pub pan_x: f64,
    #[serde(default)]
    pub pan_y: f64,
    #[serde(default = "default_zoom")]
    pub zoom: f64,
}

fn default_zoom() -> f64 { 1.0 }

/// 单个画布的数据
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PinboardCanvas {
    #[serde(default)]
    pub pins: Vec<PinInfo>,
    #[serde(default)]
    pub viewport: Option<PinboardViewport>,
    #[serde(default)]
    pub annotations: Vec<PinAnnotation>,
}

/// .pgb1_pinboard.json 根结构（key → 画布）
pub type PinboardData = std::collections::HashMap<String, PinboardCanvas>;
