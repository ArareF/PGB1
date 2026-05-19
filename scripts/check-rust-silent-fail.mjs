#!/usr/bin/env node
/**
 * Rust 关键操作静默失败扫描脚本
 *
 * 底层逻辑：`let _ = fs::write(...)` / `let _ = fs::remove_file(...)` 这类
 * 静默丢弃 Result 的写法，一旦关键操作失败，用户和日志都不会知情。
 * 本脚本扫 .rs 文件，针对文件系统 / IPC emit / 数据库等副作用 API 报警。
 *
 * 颗粒度说明：
 *   - 硬错：关键状态写回失败被吞（fs::write / fs::rename / fs::create_dir_all）
 *   - 警告：清理类失败被吞（fs::remove_file / fs::remove_dir_all）→ 建议 log
 *   - 信息：UI-emit 失败被吞（app.emit）→ 通常可接受但标记供审计
 *
 * 退出码：
 *   0 — 无硬错
 *   1 — 有硬错（阻断 build）
 */
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, extname, relative, resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const TAURI_SRC = join(ROOT, 'src-tauri', 'src')

const QUIET = process.argv.includes('--quiet')

function collectRustFiles(dir, out = []) {
  for (const entry of readdirSync(dir)) {
    const p = join(dir, entry)
    const s = statSync(p)
    if (s.isDirectory()) {
      if (entry === 'target' || entry.startsWith('.')) continue
      collectRustFiles(p, out)
    } else if (extname(entry) === '.rs') {
      out.push(p)
    }
  }
  return out
}

/**
 * 分三档：
 *   hard — 关键状态写入失败被吞（数据丢失/错乱）
 *   cleanup — 清理类失败被吞（磁盘膨胀/残留）
 *   emit — UI 事件发送失败（通常可接受）
 *
 * 正则匹配 `let _ = OP(...)` 和 `let _ = IDENT.OP(...)` 形式
 */
const PATTERNS = {
  hard: [
    { re: /let\s+_\s*=\s*fs::write\s*\(/, name: 'fs::write' },
    { re: /let\s+_\s*=\s*fs::rename\s*\(/, name: 'fs::rename' },
    { re: /let\s+_\s*=\s*fs::create_dir(_all)?\s*\(/, name: 'fs::create_dir' },
    { re: /let\s+_\s*=\s*fs::copy\s*\(/, name: 'fs::copy' },
    { re: /let\s+_\s*=\s*fs::set_permissions\s*\(/, name: 'fs::set_permissions' },
  ],
  cleanup: [
    { re: /let\s+_\s*=\s*fs::remove_file\s*\(/, name: 'fs::remove_file' },
    { re: /let\s+_\s*=\s*fs::remove_dir(_all)?\s*\(/, name: 'fs::remove_dir' },
  ],
  emit: [
    { re: /let\s+_\s*=\s*[\w.]+\.emit\s*\(/, name: '.emit' },
  ],
}

function scanFile(file) {
  const content = readFileSync(file, 'utf-8')
  const lines = content.split('\n')
  const findings = { hard: [], cleanup: [], emit: [] }
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    for (const [tier, patterns] of Object.entries(PATTERNS)) {
      for (const { re, name } of patterns) {
        if (re.test(line)) {
          findings[tier].push({ line: i + 1, name, text: line.trim() })
          break
        }
      }
    }
  }
  return findings
}

function main() {
  const files = collectRustFiles(TAURI_SRC)
  const all = { hard: [], cleanup: [], emit: [] }

  for (const file of files) {
    const findings = scanFile(file)
    for (const tier of ['hard', 'cleanup', 'emit']) {
      for (const f of findings[tier]) {
        all[tier].push({ file: relative(ROOT, file), ...f })
      }
    }
  }

  if (!QUIET) {
    console.log(`[check-rust-silent-fail] 扫描 ${files.length} 个 Rust 文件`)
    console.log('')
  }

  if (all.hard.length === 0) {
    if (!QUIET) console.log('✅ 无关键写入操作被静默丢弃')
  } else {
    console.log(`❌ 发现 ${all.hard.length} 处关键写入操作被静默丢弃（数据完整性风险）：`)
    for (const f of all.hard) {
      console.log(`  ${f.file}:${f.line}  [${f.name}]`)
      console.log(`    ${f.text.substring(0, 100)}`)
    }
    console.log('')
  }

  if (all.cleanup.length > 0 && !QUIET) {
    console.log(`⚠️  ${all.cleanup.length} 处清理类操作被静默丢弃（磁盘残留风险，建议补 log::warn!）：`)
    const byFile = new Map()
    for (const f of all.cleanup) {
      if (!byFile.has(f.file)) byFile.set(f.file, [])
      byFile.get(f.file).push(`${f.line}:${f.name}`)
    }
    for (const [file, items] of [...byFile.entries()].sort()) {
      console.log(`    ${file}  (${items.length} 处)`)
      for (const it of items.slice(0, 3)) console.log(`      :${it}`)
      if (items.length > 3) console.log(`      ... +${items.length - 3}`)
    }
    console.log('')
  }

  if (all.emit.length > 0 && !QUIET) {
    console.log(`ℹ️  ${all.emit.length} 处 .emit() 被静默丢弃（UI 事件通常可接受）`)
    console.log('')
  }

  process.exit(all.hard.length > 0 ? 1 : 0)
}

main()
