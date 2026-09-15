/**
 * 视频播放器常量 SSOT（VideoPlayer / useVideoCompare 共用）
 */

/** 逐帧步进的时间长度。预览视频帧率不可知，按 30fps 估算 */
export const VIDEO_FRAME_STEP_SEC = 1 / 30

/** 对比偏移的粗调步长（秒） */
export const VIDEO_OFFSET_STEP_SEC = 1

/**
 * 对比播放时 B 路允许的漂移容差（秒）。
 * 播放中只在超出容差时才纠偏 seek，否则每帧 seek 会让 B 路卡顿。≈ 30fps 下 2~3 帧。
 */
export const VIDEO_SYNC_DRIFT_TOLERANCE_SEC = 0.08

/**
 * 对比播放时的漂移巡检间隔（毫秒）。
 * 用 setInterval 而不是 requestAnimationFrame：rAF 在 WebView 不合成的时候（窗口被遮 / 截图态）会整个停摆，
 * 巡检一停 B 路就自由漂移；100ms 一查配合上面的容差已经足够。
 */
export const VIDEO_SYNC_CHECK_INTERVAL_MS = 100
