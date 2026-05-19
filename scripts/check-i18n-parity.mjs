#!/usr/bin/env node
/**
 * i18n 键完整性对账脚本（Claude N-27）
 *
 * 底层逻辑：src/locales/zh-CN.ts 和 en.ts 必须键完全一致。
 * 任何一方缺键 → 运行时 $t() 会返回 key 字面量，UI 直接穿帮。
 *
 * 本脚本不依赖 TS 编译，直接用缩进-嵌套结构解析：
 *   - 行匹配 `^(\s+)(\w+):`（冒号开头为键，缩进 = 嵌套深度）
 *   - 结合 `{` / `}` 跟踪嵌套路径
 *   - 扁平化为 `common.cancel`, `onboarding.startUsing`, ...
 *
 * 退出码：
 *   0 — 两文件键集一致
 *   1 — 键集不匹配
 */
import { readFileSync } from 'node:fs'
import { join, resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const LOCALES_DIR = join(ROOT, 'src', 'locales')

const QUIET = process.argv.includes('--quiet')

/**
 * 解析一个 locale 文件的键集。
 * 返回 Set<string>，元素为扁平化路径（`common.cancel`）。
 */
function extractKeys(content) {
  const keys = new Set()
  const lines = content.split('\n')
  // 路径栈：[(keyName, indent)]，遇到 { 进栈，遇到 } 退栈
  const stack = []

  for (let i = 0; i < lines.length; i++) {
    const rawLine = lines[i]
    // 删除行尾注释（`// ...`），避免 { } 干扰
    let line = rawLine.replace(/\/\/[^\n]*$/, '').trimEnd()
    if (line.length === 0) continue

    // 检测键声明：可选前导空白 + identifier（可带引号）+ 冒号
    const keyMatch = line.match(/^(\s*)(['"]?)([\w-]+)\2\s*:\s*(.*)$/)
    if (!keyMatch) {
      // 不是键行，只处理大括号变化（可能是 `}` 单独一行）
      const closes = (line.match(/\}/g) || []).length - (line.match(/\{/g) || []).length
      for (let k = 0; k < closes; k++) if (stack.length > 0) stack.pop()
      continue
    }

    const [, indent, , keyName, rest] = keyMatch
    // 计算当前缩进所处嵌套层级：弹出比自己缩进大或等于的栈
    while (stack.length > 0 && stack[stack.length - 1].indent >= indent.length) {
      stack.pop()
    }

    const path = [...stack.map(s => s.key), keyName].join('.')

    // 判断这个键的值是子对象还是叶子
    // 如果 rest 开头是 `{`（可能后面跟着 } 同行表示空对象），则是对象
    const trimRest = rest.trimStart()
    if (trimRest.startsWith('{')) {
      // 统计同行的开闭 {/}
      const restOpens = (trimRest.match(/\{/g) || []).length
      const restCloses = (trimRest.match(/\}/g) || []).length
      if (restOpens > restCloses) {
        // 进栈
        stack.push({ key: keyName, indent: indent.length })
      }
      // 否则是 `a: { ... },` 单行对象（罕见）不扩展
    } else {
      // 叶子键
      keys.add(path)
    }
  }
  return keys
}

function main() {
  const zhPath = join(LOCALES_DIR, 'zh-CN.ts')
  const enPath = join(LOCALES_DIR, 'en.ts')

  let zh, en
  try {
    zh = readFileSync(zhPath, 'utf-8')
    en = readFileSync(enPath, 'utf-8')
  } catch (e) {
    console.error(`❌ locale 文件读取失败: ${e.message}`)
    process.exit(1)
  }

  const zhKeys = extractKeys(zh)
  const enKeys = extractKeys(en)

  const missingInEn = [...zhKeys].filter(k => !enKeys.has(k)).sort()
  const missingInZh = [...enKeys].filter(k => !zhKeys.has(k)).sort()

  if (!QUIET) {
    console.log(`[check-i18n-parity] zh-CN 键数: ${zhKeys.size}`)
    console.log(`[check-i18n-parity] en    键数: ${enKeys.size}`)
    console.log('')
  }

  if (missingInEn.length === 0 && missingInZh.length === 0) {
    if (!QUIET) console.log('✅ 两个 locale 键集完全一致')
    process.exit(0)
  }

  if (missingInEn.length > 0) {
    console.log(`❌ en.ts 缺失 ${missingInEn.length} 个键（zh-CN 有但 en 无）：`)
    for (const k of missingInEn.slice(0, 30)) console.log(`    ${k}`)
    if (missingInEn.length > 30) console.log(`    ... +${missingInEn.length - 30}`)
    console.log('')
  }

  if (missingInZh.length > 0) {
    console.log(`❌ zh-CN.ts 缺失 ${missingInZh.length} 个键（en 有但 zh-CN 无）：`)
    for (const k of missingInZh.slice(0, 30)) console.log(`    ${k}`)
    if (missingInZh.length > 30) console.log(`    ... +${missingInZh.length - 30}`)
    console.log('')
  }

  process.exit(1)
}

main()
