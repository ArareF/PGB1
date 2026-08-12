import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..')
const read = path => readFileSync(resolve(root, path), 'utf8')

const designSystem = read('src/styles/design-system.css')
const sharpGrid = read('src/styles/sharp-grid.css')
const homePage = read('src/views/HomePage.vue')
const useTheme = read('src/composables/useTheme.ts')

const failures = []
const expect = (condition, message) => {
  if (!condition) failures.push(message)
}

expect(
  /:root\[data-theme="dark"\][\s\S]*--sg-bg-work:/.test(designSystem),
  'Sharp Grid 主题 token 必须进入 design-system.css 的深色主题变量层',
)
expect(
  !/^\s*\.sharp-grid(?:\s|,|\{)/m.test(sharpGrid),
  'sharp-grid.css 不应再依赖主页局部 .sharp-grid 作用域',
)
expect(
  sharpGrid.includes(':root[data-theme="dark"]'),
  'sharp-grid.css 必须由全局深色主题属性驱动',
)
expect(
  !homePage.includes('pgb1-home-sharpgrid') && !homePage.includes('sg-toggle'),
  'HomePage 必须移除临时原版/精装试点开关',
)
expect(
  /ref<Theme>\('dark'\)/.test(useTheme) && /saved \|\| 'dark'/.test(useTheme),
  '新用户与无历史偏好场景必须默认进入已升级的深色主题',
)

const requiredCoverage = [
  '.main-layout',
  '.title-bar-center',
  '.sidebar',
  '.project-card',
  '.task-card',
  '.material-card',
  '.normal-card',
  '.dialog-content',
  '.sidebar-shell',
  '.task-page',
  '.settings-page',
  '.reminder-container',
  '.translator-window',
  '.pinboard-page',
]

for (const selector of requiredCoverage) {
  expect(sharpGrid.includes(selector), `缺少全局深色覆盖：${selector}`)
}

expect(
  sharpGrid.includes('@media (prefers-reduced-motion: reduce)'),
  'Sharp Grid 全局主题必须为减少动态效果偏好提供降级',
)

if (failures.length > 0) {
  console.error(`Sharp Grid 覆盖检查失败（${failures.length} 项）：`)
  for (const failure of failures) console.error(`- ${failure}`)
  process.exit(1)
}

console.log(`Sharp Grid 覆盖检查通过：${requiredCoverage.length} 个关键界面锚点已覆盖。`)
