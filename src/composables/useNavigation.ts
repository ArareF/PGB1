import { ref, readonly } from 'vue'
import type { Ref } from 'vue'
import { APP_NAME } from '../config/app'

/* 快捷功能按钮定义 */
export interface NavAction {
  id: string
  label: string
  icon?: string      /* 预留：SVG 图标名，Phase 3+ 实现图标系统 */
  handler: () => void
  onLongPress?: () => void  /* 长按 500ms 触发，有此字段时单击取最新，长按弹选择 */
  active?: boolean          /* 按钮强调状态（如已设默认时的「打开AE」） */
  disabled?: boolean
}

/* 更多菜单项定义 */
export interface MoreMenuItem {
  id: string
  label: string
  handler: () => void
}

/* 导航状态 — 模块级单例（和 useTheme 一致） */
const title = ref(APP_NAME)
const showBackButton = ref(false)
const backHandler = ref<(() => void) | null>(null)
const actions = ref<NavAction[]>([])
const moreMenuItems = ref<MoreMenuItem[]>([])
const routeDirection = ref<'forward' | 'back'>('forward')

export function useNavigation() {
  /**
   * 页面调用此方法注册导航配置。
   * 每次路由切换时由新页面重新调用，覆盖上一个页面的配置。
   */
  function setNavigation(config: {
    title: string
    showBackButton?: boolean
    onBack?: () => void
    actions?: NavAction[]
    moreMenuItems?: MoreMenuItem[]
  }) {
    title.value = config.title
    showBackButton.value = config.showBackButton ?? false
    backHandler.value = config.onBack ?? null
    actions.value = config.actions ?? []
    moreMenuItems.value = config.moreMenuItems ?? []
  }

  function goBack() {
    if (backHandler.value) {
      backHandler.value()
    }
  }

  function setRouteDirection(dir: 'forward' | 'back') {
    routeDirection.value = dir
  }

  return {
    /* 只读状态 — TitleBar 和更多菜单消费 */
    title: readonly(title) as Readonly<Ref<string>>,
    showBackButton: readonly(showBackButton) as Readonly<Ref<boolean>>,
    actions: readonly(actions) as Readonly<Ref<readonly NavAction[]>>,
    moreMenuItems: readonly(moreMenuItems) as Readonly<Ref<readonly MoreMenuItem[]>>,
    routeDirection: readonly(routeDirection) as Readonly<Ref<'forward' | 'back'>>,

    /* 写入方法 — 页面组件调用 */
    setNavigation,
    setRouteDirection,
    goBack,
  }
}
