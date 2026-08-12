/**
 * 素材系列聚合 SSOT
 *
 * 业务背景：原画组交付的设计稿按 `{基础名}_{YYMMDD}.{扩展名}` 命名，同一个素材
 * 会随时间陆续交付多版（Symbols_251209.psd → Symbols_260313.psd → …）。
 * 项目素材页平铺展示会被历史版本淹没，这里把同系列聚合成一张卡。
 *
 * 纯前端展示层聚合：`scan_directory` 已返回目录下全部文件，不需要 Rust 参与。
 */

import type { FileEntry } from '../composables/useDirectoryFiles'
import { IMAGE_EXTS_BROWSE, PSD_EXTS } from '../config/fileTypes'

/** 版本日期的位数：YYMMDD */
const DATE_TOKEN_LENGTH = 6
/** 分组键归一化时要抹掉的分隔符（`Bonus Boost_UI` 与 `BonusBoost_UI` 视为同系列） */
const SEPARATOR_PATTERN = /[\s_-]/g
/** 文件名 token 分隔符 */
const TOKEN_SEPARATOR = '_'

/** 单个版本：同一天交付的所有格式算一个版本 */
export interface SeriesVersion {
  /** 版本日期 YYMMDD；无日期文件为空串 */
  date: string
  /** 该日期下的所有文件（psd / jpg 等），primary 优先序 */
  files: FileEntry[]
}

/** 一个素材系列 = 项目素材页上的一张卡 */
export interface MaterialSeries {
  /** 归一化分组键，同时用作渲染 key */
  key: string
  /** 显示名：最新版本文件的原始基础名（保留原始大小写） */
  label: string
  /** 版本列表，日期倒序（最新在前） */
  versions: SeriesVersion[]
  /** 缩略图取样文件：优先图片格式（jpg 秒出，不必走 PSD 提取） */
  cover: FileEntry
  /** 交互主体：点击 / 拖拽 / 重命名的目标，优先 PSD 工作文件 */
  primary: FileEntry
  /** 系列内文件总数（跨版本跨格式） */
  fileCount: number
}

/** 解析结果：基础名 + 版本日期 */
export interface ParsedName {
  baseName: string
  date: string
  /**
   * 日期 token 之后的残留（含前导下划线），如 `MainBonus_UI_260706_---` 的 `_---`。
   * 不参与分组，但同一天存在多个同格式文件时靠它区分（否则版本列表两行长得一样）。
   */
  suffix: string
}

/** 6 位数字 token 是否为合法 YYMMDD（月 01-12、日 01-31） */
function isDateToken(token: string): boolean {
  if (token.length !== DATE_TOKEN_LENGTH || !/^\d+$/.test(token)) return false
  const month = Number(token.slice(2, 4))
  const day = Number(token.slice(4, 6))
  return month >= 1 && month <= 12 && day >= 1 && day <= 31
}

/** 去掉文件名末尾的扩展名 */
function getStem(file: FileEntry): string {
  if (!file.extension) return file.name
  return file.name.slice(0, -(file.extension.length + 1))
}

/**
 * 从文件名解析基础名与版本日期。
 *
 * 从右往左找第一个合法日期 token —— 右侧优先是因为日期是后缀，
 * 这样 `MainBonus_UI_260706_---` 能正确落到 `260706`（尾部噪音丢弃），
 * 基础名里恰好含 6 位数字时也不会被误当版本。
 *
 * 找不到日期返回 null（无版本文件，各自独立成卡）。
 */
export function parseSeriesName(file: FileEntry): ParsedName | null {
  const tokens = getStem(file).split(TOKEN_SEPARATOR)
  for (let i = tokens.length - 1; i >= 0; i--) {
    if (!isDateToken(tokens[i])) continue
    const baseName = tokens.slice(0, i).join(TOKEN_SEPARATOR)
    // 日期是整个 stem 的开头（如 `260807.psd`）时没有基础名，不算系列
    if (!baseName) return null
    const rest = tokens.slice(i + 1)
    return {
      baseName,
      date: tokens[i],
      suffix: rest.length > 0 ? TOKEN_SEPARATOR + rest.join(TOKEN_SEPARATOR) : '',
    }
  }
  return null
}

/** 分组键：忽略大小写与空格 / 下划线 / 连字符 */
export function seriesKey(baseName: string): string {
  return baseName.toLowerCase().replace(SEPARATOR_PATTERN, '')
}

/** 格式优先级：数字越小越优先被选作该角色的代表文件 */
function formatRank(file: FileEntry, prefer: 'image' | 'psd'): number {
  const ext = file.extension.toLowerCase()
  const isImage = IMAGE_EXTS_BROWSE.has(ext)
  const isPsd = PSD_EXTS.has(ext)
  if (prefer === 'image') {
    if (isImage) return 0
    if (isPsd) return 1
  } else {
    if (isPsd) return 0
    if (isImage) return 1
  }
  return 2
}

/** 在候选文件里挑出指定角色的代表文件 */
function pickRepresentative(files: FileEntry[], prefer: 'image' | 'psd'): FileEntry {
  return files.reduce((best, f) =>
    formatRank(f, prefer) < formatRank(best, prefer) ? f : best
  )
}

/** 把单个文件包成只有一版的系列（目录、无日期文件走这条路） */
function toSingletonSeries(file: FileEntry): MaterialSeries {
  return {
    key: file.path,
    label: file.name,
    versions: [{ date: '', files: [file] }],
    cover: file,
    primary: file,
    fileCount: 1,
  }
}

/**
 * 把目录扫描结果聚合成系列列表。
 *
 * 规则（已与产品对齐）：
 * - 目录不参与合并，原样单卡（点击仍进文件夹浏览弹窗）
 * - 无日期后缀的文件不参与合并，原样单卡
 * - 同一日期的多个格式合并成一个版本条目
 * - 系列内版本按日期倒序；系列之间按最新版本日期倒序，无日期的排最后
 *
 * 已知不解决：`winscreen` / `winscreens` 这类词形差异会各自成卡，
 * 由用户在侧边栏重命名收敛 —— 不引入第二套分组元数据，SSOT 是文件名本身。
 */
export function groupIntoSeries(files: FileEntry[]): MaterialSeries[] {
  const grouped = new Map<string, Map<string, FileEntry[]>>()
  const baseNames = new Map<string, Map<string, string>>()
  const singletons: MaterialSeries[] = []

  for (const file of files) {
    const parsed = file.is_dir ? null : parseSeriesName(file)
    if (!parsed) {
      singletons.push(toSingletonSeries(file))
      continue
    }
    const key = seriesKey(parsed.baseName)
    let byDate = grouped.get(key)
    if (!byDate) {
      byDate = new Map()
      grouped.set(key, byDate)
      baseNames.set(key, new Map())
    }
    const bucket = byDate.get(parsed.date)
    if (bucket) bucket.push(file)
    else byDate.set(parsed.date, [file])
    // 记下每个日期对应的原始基础名，供显示名取最新版写法
    baseNames.get(key)!.set(parsed.date, parsed.baseName)
  }

  const series: MaterialSeries[] = []
  for (const [key, byDate] of grouped) {
    const versions: SeriesVersion[] = [...byDate.entries()]
      .sort((a, b) => b[0].localeCompare(a[0]))
      .map(([date, group]) => ({
        date,
        // 版本内也排序，让 primary 优先的格式排在前面
        files: [...group].sort((a, b) => formatRank(a, 'psd') - formatRank(b, 'psd')),
      }))

    const latest = versions[0]
    series.push({
      key,
      label: baseNames.get(key)!.get(latest.date)!,
      versions,
      cover: pickRepresentative(latest.files, 'image'),
      primary: pickRepresentative(latest.files, 'psd'),
      fileCount: versions.reduce((sum, v) => sum + v.files.length, 0),
    })
  }

  // 最近动过的排前面；无日期的单卡按原扫描顺序缀在最后
  series.sort((a, b) => b.versions[0].date.localeCompare(a.versions[0].date))
  return [...series, ...singletons]
}

/** 系列内全部文件扁平化，日期倒序（供侧边栏版本列表使用） */
export function flattenVersions(series: MaterialSeries): FileEntry[] {
  return series.versions.flatMap(v => v.files)
}
