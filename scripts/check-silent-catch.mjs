#!/usr/bin/env node
/**
 * 前端静默 catch 扫描脚本
 *
 * 底层逻辑：`} catch (e) {}` / `} catch {}` 这种空实现吞异常，
 * UI 看起来只是"没反应"但维护者拿不到任何原因。
 * 本脚本扫 `.ts / .vue` 源文件，用 AST-轻量 的正则找空 catch 块。
 *
 * 颗粒度说明：
 *   - 空 catch（catch {} / catch(e) {} / catch(_) {}） → 硬错
 *   - 注释-only catch（catch 块内只有注释）→ 软兜底（列出但不阻断）
 *   - 带 console / log / emit 等任一副作用的 → 不报（有反馈）
 *
 * 退出码：
 *   0 — 无硬错静默 catch（允许软兜底存在）
 *   1 — 有硬错
 *
 * 用法：
 *   node scripts/check-silent-catch.mjs
 *   node scripts/check-silent-catch.mjs --threshold 20   允许最多 N 个（用于渐进收敛）
 */
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, extname, relative, resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const SRC_DIR = join(ROOT, 'src')

const args = process.argv.slice(2)
const QUIET = args.includes('--quiet')
const thresholdIdx = args.indexOf('--threshold')
const THRESHOLD = thresholdIdx !== -1 ? parseInt(args[thresholdIdx + 1], 10) : 0

function collectFiles(dir, out = []) {
  for (const entry of readdirSync(dir)) {
    const p = join(dir, entry)
    const s = statSync(p)
    if (s.isDirectory()) {
      if (entry === 'node_modules' || entry.startsWith('.')) continue
      collectFiles(p, out)
    } else {
      const ext = extname(entry)
      if (ext === '.ts' || ext === '.tsx' || ext === '.vue' || ext === '.js') out.push(p)
    }
  }
  return out
}

// 匹配 catch 块：`} catch (...) {`  或 `} catch {`
// 然后分析块内内容，判断是否为空/注释-only/有副作用
const CATCH_RE = /\bcatch\s*(?:\([^)]*\))?\s*\{/g

function extractCatchBodies(content) {
  const bodies = []
  let match
  const re = new RegExp(CATCH_RE.source, 'g')
  while ((match = re.exec(content)) !== null) {
    const openIdx = match.index + match[0].length - 1 // 指向 {
    // 找到匹配的 }
    let depth = 1
    let i = openIdx + 1
    let inString = null
    let inComment = null
    while (i < content.length && depth > 0) {
      const ch = content[i]
      const next = content[i + 1]
      if (inComment === 'line') {
        if (ch === '\n') inComment = null
      } else if (inComment === 'block') {
        if (ch === '*' && next === '/') { inComment = null; i++ }
      } else if (inString) {
        if (ch === '\\') { i++ } // skip escaped char
        else if (ch === inString) inString = null
      } else {
        if (ch === '/' && next === '/') { inComment = 'line'; i++ }
        else if (ch === '/' && next === '*') { inComment = 'block'; i++ }
        else if (ch === '"' || ch === "'" || ch === '`') inString = ch
        else if (ch === '{') depth++
        else if (ch === '}') depth--
      }
      i++
    }
    const body = content.substring(openIdx + 1, i - 1)
    // 行号
    const line = content.substring(0, match.index).split('\n').length
    bodies.push({ body, line, matchText: match[0] })
  }
  return bodies
}

/**
 * 判定 catch 块类型：
 *   'empty'       — 完全空或只有空白（硬错）
 *   'comment'     — 只有注释（软兜底）
 *   'no-logging'  — 有副作用但无日志（UI 看到结果变化但拿不到原因，GPT P3-06 口径）
 *   'handled'     — 有 console / log / emit 等日志输出
 */
function classifyCatchBody(body) {
  // 移除注释
  const noComment = body
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/\/\/[^\n]*/g, '')
    .trim()

  if (noComment.length === 0) {
    // 全是注释或空白
    return body.trim().length === 0 ? 'empty' : 'comment'
  }
  // 判断是否有日志副作用（console.* / log.* / logger.* / emit error / throw / alert）
  const hasLogging = /\b(console\.\w+|log\.\w+|logger\.\w+|window\.\w*log|throw\b|alert\()/i.test(noComment)
  return hasLogging ? 'handled' : 'no-logging'
}

function main() {
  const files = collectFiles(SRC_DIR)
  const hardErrors = []    // 完全空 catch
  const softWarnings = []  // 注释-only catch
  const noLogging = []     // 有副作用但无日志（GPT P3-06 口径）

  for (const file of files) {
    const content = readFileSync(file, 'utf-8')
    const bodies = extractCatchBodies(content)
    for (const b of bodies) {
      const cls = classifyCatchBody(b.body)
      const loc = { file: relative(ROOT, file), line: b.line }
      if (cls === 'empty') hardErrors.push(loc)
      else if (cls === 'comment') softWarnings.push(loc)
      else if (cls === 'no-logging') noLogging.push(loc)
    }
  }

  if (!QUIET) {
    console.log(`[check-silent-catch] 扫描 ${files.length} 个前端源文件`)
    console.log('')
  }

  if (hardErrors.length === 0) {
    if (!QUIET) console.log('✅ 无完全空的 catch 块')
  } else {
    console.log(`❌ 发现 ${hardErrors.length} 处完全空的 catch（UI 失败时用户 + 维护者都拿不到原因）：`)
    // 按文件分组
    const byFile = new Map()
    for (const e of hardErrors) {
      if (!byFile.has(e.file)) byFile.set(e.file, [])
      byFile.get(e.file).push(e.line)
    }
    for (const [file, lines] of [...byFile.entries()].sort()) {
      console.log(`  ${file}  (${lines.length} 处)`)
      for (const line of lines.slice(0, 3)) console.log(`    :${line}`)
      if (lines.length > 3) console.log(`    ... +${lines.length - 3}`)
    }
    console.log('')
  }

  if (noLogging.length > 0 && !QUIET) {
    console.log(`⚠️  ${noLogging.length} 处 catch 有副作用但无日志（UI 看到结果变化但拿不到原因 — GPT P3-06）：`)
    const byFile = new Map()
    for (const w of noLogging) {
      if (!byFile.has(w.file)) byFile.set(w.file, [])
      byFile.get(w.file).push(w.line)
    }
    for (const [file, lines] of [...byFile.entries()].sort()) {
      console.log(`    ${file}  (${lines.length} 处) 行 ${lines.slice(0, 5).join(', ')}${lines.length > 5 ? ' ...' : ''}`)
    }
    console.log('')
  }

  if (softWarnings.length > 0 && !QUIET) {
    console.log(`ℹ️  ${softWarnings.length} 处 catch 仅注释无副作用（知情忽略，软兜底）：`)
    const byFile = new Map()
    for (const w of softWarnings) {
      if (!byFile.has(w.file)) byFile.set(w.file, [])
      byFile.get(w.file).push(w.line)
    }
    for (const [file, lines] of [...byFile.entries()].sort()) {
      console.log(`    ${file}  (${lines.length} 处)`)
    }
    console.log('')
  }

  // 阈值模式：允许渐进收敛（Sprint 4-B 起设 THRESHOLD=16，修一处降一处）
  if (THRESHOLD > 0) {
    if (hardErrors.length > THRESHOLD) {
      console.log(`❌ 静默 catch 数 ${hardErrors.length} 超过阈值 ${THRESHOLD}`)
      process.exit(1)
    }
    if (!QUIET && hardErrors.length > 0) {
      console.log(`ℹ️  当前 ${hardErrors.length} 个 ≤ 阈值 ${THRESHOLD}（渐进收敛模式）`)
    }
    process.exit(0)
  }

  process.exit(hardErrors.length > 0 ? 1 : 0)
}

main()
