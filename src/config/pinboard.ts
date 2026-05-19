/**
 * 贴图板（Pinboard）画笔预设
 *
 * 从 PinboardPage.vue 提取（N-24）：让颜色和尺寸成为显式配置，
 * 未来要加主题切换 / 新增颜色时不用改页面代码。
 */

/** 画笔预设颜色（红/蓝/绿/橙/白，覆盖常见标注场景） */
export const PINBOARD_PEN_COLORS = [
  { name: 'red',    hex: '#FF3B30' },
  { name: 'blue',   hex: '#007AFF' },
  { name: 'green',  hex: '#34C759' },
  { name: 'orange', hex: '#FF9500' },
  { name: 'white',  hex: '#FFFFFF' },
] as const

/** 纯 hex 数组（与旧代码兼容） */
export const PINBOARD_PEN_COLOR_HEXES = PINBOARD_PEN_COLORS.map(c => c.hex)

/** 工具尺寸范围 */
export const PINBOARD_TOOL_SIZE = {
  pen:    { min: 1,  max: 20, default: 3 },
  arrow:  { min: 1,  max: 20, default: 3 },
  rect:   { min: 1,  max: 20, default: 3 },
  ellipse:{ min: 1,  max: 20, default: 3 },
  eraser: { min: 5,  max: 50, default: 20 },
  text:   { min: 10, max: 48, default: 16 },
} as const
