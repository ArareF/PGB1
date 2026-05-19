#!/usr/bin/env node
/**
 * 版本号多文件同步对账脚本
 *
 * 底层逻辑：项目里 6 个位置声明了版本号，发版时任一处漏改就是 Y-6 类回归。
 * 本脚本把 source of truth 定为 package.json.version，其他 5 处必须与之一致。
 *
 * 被守护的位置：
 *   - package.json                  "version": "X.Y.Z"
 *   - src-tauri/Cargo.toml          version = "X.Y.Z"
 *   - src-tauri/tauri.conf.json     "version": "X.Y.Z"
 *   - src/config/app.ts             APP_VERSION = 'VX.Y.Z'
 *   - README.md                     **当前版本**：VX.Y.Z
 *   - INDEX.md                      已发布 vX.Y.Z
 *   - latest.json                   "version": "X.Y.Z"（发版清单）
 *
 * 退出码：
 *   0 — 所有位置版本号一致
 *   1 — 存在版本号漂移
 */
import { readFileSync } from 'node:fs'
import { join, resolve, dirname, relative } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')

const QUIET = process.argv.includes('--quiet')

/**
 * 被检查的 6 个位置 + 1 个可选发版清单。
 * pattern 用捕获组抓取版本号字符串（X.Y.Z 格式）。
 */
const CHECKS = [
  {
    file: 'package.json',
    pattern: /"version"\s*:\s*"([\d.]+)"/,
    required: true,
    isSourceOfTruth: true,
  },
  {
    file: 'src-tauri/Cargo.toml',
    pattern: /^\s*version\s*=\s*"([\d.]+)"/m,
    required: true,
  },
  {
    file: 'src-tauri/tauri.conf.json',
    pattern: /"version"\s*:\s*"([\d.]+)"/,
    required: true,
  },
  {
    file: 'src/config/app.ts',
    pattern: /APP_VERSION\s*=\s*'V([\d.]+)'/,
    required: true,
    transform: v => v, // 前端带 V 前缀，这里只对比数字部分
  },
  {
    file: 'README.md',
    pattern: /\*\*当前版本\*\*[：:]\s*V([\d.]+)/,
    required: true,
  },
  {
    file: 'INDEX.md',
    pattern: /已发布\s*v([\d.]+)/,
    required: true,
  },
  {
    file: 'latest.json',
    pattern: /"version"\s*:\s*"([\d.]+)"/,
    required: false, // 发版清单可能暂时落后，只警告不阻断
  },
]

function extractVersion(check) {
  const path = join(ROOT, check.file)
  let content
  try {
    content = readFileSync(path, 'utf-8')
  } catch (e) {
    return { error: `读取失败: ${e.message}` }
  }
  const match = content.match(check.pattern)
  if (!match) return { error: '未匹配到版本号' }
  return { version: match[1] }
}

function main() {
  const results = CHECKS.map(c => ({ ...c, ...extractVersion(c) }))

  const truth = results.find(r => r.isSourceOfTruth)
  if (!truth || truth.error) {
    console.error('❌ package.json 版本号读取失败，无法对账')
    console.error(`   ${truth?.error || '未定义 source of truth'}`)
    process.exit(1)
  }

  const expected = truth.version
  const drifts = []
  const warnings = []
  const missing = []

  for (const r of results) {
    if (r.isSourceOfTruth) continue
    if (r.error) {
      if (r.required) missing.push({ file: r.file, reason: r.error })
      continue
    }
    if (r.version !== expected) {
      if (r.required) drifts.push({ file: r.file, actual: r.version })
      else warnings.push({ file: r.file, actual: r.version })
    }
  }

  if (!QUIET) {
    console.log(`[check-version-sync] source of truth (package.json): ${expected}`)
    console.log(`[check-version-sync] 检查 ${CHECKS.length} 个位置`)
    console.log('')
  }

  if (drifts.length === 0 && missing.length === 0) {
    if (!QUIET) console.log(`✅ 所有版本号一致：${expected}`)
  } else {
    if (drifts.length > 0) {
      console.log(`❌ 发现 ${drifts.length} 处版本漂移（期望 ${expected}）：`)
      for (const d of drifts) console.log(`    ${d.file} → ${d.actual}`)
      console.log('')
    }
    if (missing.length > 0) {
      console.log(`❌ 发现 ${missing.length} 处版本号读取失败：`)
      for (const m of missing) console.log(`    ${m.file}: ${m.reason}`)
      console.log('')
    }
  }

  if (warnings.length > 0 && !QUIET) {
    console.log(`⚠️  非必需位置版本落后（如 latest.json 发版清单可能滞后）：`)
    for (const w of warnings) console.log(`    ${w.file} → ${w.actual}`)
    console.log('')
  }

  process.exit(drifts.length + missing.length > 0 ? 1 : 0)
}

main()
