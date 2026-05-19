#!/usr/bin/env node
/**
 * CSS 设计系统 token 对账脚本
 *
 * 底层逻辑：从全仓 .css / .vue 文件中提取
 *   - 定义集：所有 `--token-name:` 的声明
 *   - 引用集：所有 `var(--token-name[, fallback])` 的使用
 * 计算差集，报告"引用但未定义"的 token。
 *
 * 用途（GPT P1-01 / Sprint 4-A A.7）：
 *   v2.8.11 基线实测存在 12+ 个未定义 token、127+ 处活跃引用。
 *   此脚本作为 build 前对账，防止 DesignSystem "定义集 vs 消费集" 继续分叉。
 *
 * 退出码：
 *   0 — 所有引用都有定义
 *   1 — 发现未定义 token
 *
 * 用法：
 *   node scripts/check-css-tokens.mjs
 *   node scripts/check-css-tokens.mjs --quiet   仅输出问题，不输出统计
 *   node scripts/check-css-tokens.mjs --json    输出 JSON 结构
 */
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, extname, relative, resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

// 防路径编码坑：Windows 路径含空格时 import.meta.url 的 pathname 会 URL 编码
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const SRC_DIR = join(ROOT, 'src')

const args = process.argv.slice(2)
const QUIET = args.includes('--quiet')
const JSON_OUT = args.includes('--json')

// 忽略的 token（已知第三方或合法外部声明）
const IGNORE_LIST = new Set([
  // Vue 内部
  '--tw-', // tailwind remnants
])

// 递归收集所有 .css / .vue 文件
function collectFiles(dir, out = []) {
  for (const entry of readdirSync(dir)) {
    const p = join(dir, entry)
    const s = statSync(p)
    if (s.isDirectory()) {
      if (entry === 'node_modules' || entry.startsWith('.')) continue
      collectFiles(p, out)
    } else {
      const ext = extname(entry)
      if (ext === '.css' || ext === '.vue') out.push(p)
    }
  }
  return out
}

// 定义正则：匹配 --token-name:（行首可能有空白，冒号前可能有空白）
const DEFINE_RE = /^\s*(--[\w-]+)\s*:/gm

// 引用正则：匹配 var(--token-name) 或 var(--token-name, fallback)
const REFERENCE_RE = /var\(\s*(--[\w-]+)(?:\s*,[^)]*)?\)/g

function shouldIgnore(token) {
  for (const prefix of IGNORE_LIST) {
    if (token.startsWith(prefix)) return true
  }
  return false
}

function analyzeFile(filePath) {
  const content = readFileSync(filePath, 'utf-8')
  const defines = new Set()
  const refs = new Map() // token -> array of {line, hasFallback}

  // 定义扫描（全文档）
  let m
  const defineRe = new RegExp(DEFINE_RE.source, 'gm')
  while ((m = defineRe.exec(content)) !== null) {
    defines.add(m[1])
  }

  // 引用扫描（带行号）
  const lines = content.split('\n')
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    const refRe = new RegExp(REFERENCE_RE.source, 'g')
    while ((m = refRe.exec(line)) !== null) {
      const token = m[1]
      // 判断是否有 fallback
      const hasFallback = m[0].includes(',')
      if (!refs.has(token)) refs.set(token, [])
      refs.get(token).push({ line: i + 1, hasFallback })
    }
  }

  return { defines, refs }
}

function main() {
  const files = collectFiles(SRC_DIR)

  // 全局定义集
  const allDefines = new Set()
  // token -> [{ file, line, hasFallback }]
  const allRefs = new Map()

  for (const file of files) {
    const { defines, refs } = analyzeFile(file)
    for (const d of defines) allDefines.add(d)
    for (const [token, locations] of refs) {
      if (!allRefs.has(token)) allRefs.set(token, [])
      for (const loc of locations) {
        allRefs.get(token).push({ file: relative(ROOT, file), ...loc })
      }
    }
  }

  // 计算未定义 token（引用但无定义，且无 fallback 兜底）
  // 有 fallback 的引用属于"软兜底"，不视为 hard error，但仍列出供参考
  const undefinedHard = new Map() // no fallback → hard error
  const undefinedSoft = new Map() // has fallback → warn only

  for (const [token, locations] of allRefs) {
    if (allDefines.has(token)) continue
    if (shouldIgnore(token)) continue

    const hardLocs = locations.filter(l => !l.hasFallback)
    const softLocs = locations.filter(l => l.hasFallback)
    if (hardLocs.length > 0) undefinedHard.set(token, hardLocs)
    if (softLocs.length > 0) undefinedSoft.set(token, softLocs)
  }

  if (JSON_OUT) {
    const output = {
      definesCount: allDefines.size,
      refsCount: [...allRefs.values()].reduce((n, l) => n + l.length, 0),
      undefinedHard: Object.fromEntries(undefinedHard),
      undefinedSoft: Object.fromEntries(undefinedSoft),
    }
    console.log(JSON.stringify(output, null, 2))
  } else {
    if (!QUIET) {
      console.log(`[check-css-tokens] 扫描 ${files.length} 个文件`)
      console.log(`[check-css-tokens] 定义集: ${allDefines.size} 个 token`)
      console.log(
        `[check-css-tokens] 引用集: ${[...allRefs.values()].reduce((n, l) => n + l.length, 0)} 处引用，涉及 ${allRefs.size} 个 token`
      )
      console.log('')
    }

    if (undefinedHard.size === 0) {
      if (!QUIET) console.log('✅ 无硬错：所有未定义 token 都有 fallback 兜底')
    } else {
      console.log(`❌ 发现 ${undefinedHard.size} 个未定义 token 无 fallback（浏览器将静默丢弃样式）：`)
      console.log('')
      for (const [token, locations] of [...undefinedHard.entries()].sort()) {
        console.log(`  ${token}  (${locations.length} 处)`)
        // 只显示前 3 处，其余省略
        const preview = locations.slice(0, 3)
        for (const loc of preview) {
          console.log(`    ${loc.file}:${loc.line}`)
        }
        if (locations.length > 3) {
          console.log(`    ... +${locations.length - 3} 处`)
        }
      }
      console.log('')
    }

    if (undefinedSoft.size > 0 && !QUIET) {
      console.log(`⚠️  另有 ${undefinedSoft.size} 个未定义 token 带 fallback（软兜底，建议补齐定义）：`)
      for (const token of [...undefinedSoft.keys()].sort()) {
        console.log(`    ${token}`)
      }
      console.log('')
    }
  }

  process.exit(undefinedHard.size > 0 ? 1 : 0)
}

main()
