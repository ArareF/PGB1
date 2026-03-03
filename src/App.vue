<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useTheme } from './composables/useTheme'
import { useScale } from './composables/useScale'
import { useSettings } from './composables/useSettings'
import MainLayout from './layouts/MainLayout.vue'
import OnboardingDialog from './components/OnboardingDialog.vue'
import UpdateDialog from './components/UpdateDialog.vue'
import { useUpdater } from './composables/useUpdater'

const route = useRoute()
const router = useRouter()
const { locale } = useI18n()
const { initTheme } = useTheme()
const { initScale } = useScale()
const { settings, loadSettings } = useSettings()

const ready = ref(false)
const showOnboarding = ref(false)

onMounted(async () => {
  initTheme()
  const s = await loadSettings()
  // 语言
  locale.value = s?.general?.language || 'zh-CN'
  // uiScale: 旧用户可能存了 0（自动模式），统一降级为 100%
  const scale = (s?.general?.uiScale ?? 0) > 0 ? s!.general.uiScale : 1
  initScale(scale)

  // 首次启动检测
  showOnboarding.value = !s?.general?.onboarded
  ready.value = true

  // 非首次启动时，延迟检查更新
  if (!showOnboarding.value) {
    const { scheduleCheck } = useUpdater()
    scheduleCheck()
  }
})

async function onOnboardingComplete(mode: 'off' | 'auto' | 'record_only') {
  showOnboarding.value = false
  // 重新加载设置以刷新缓存（引导过程中已保存 onboarded=true）
  // 清除缓存强制重载
  settings.value = null
  const s = await loadSettings()
  // 重新应用语言和缩放（引导可能修改了这些值）
  locale.value = s?.general?.language || 'zh-CN'
  const scale = (s?.general?.uiScale ?? 0) > 0 ? s!.general.uiScale : 1
  initScale(scale)

  // 开启了打卡 → 跳转设置页出勤 Tab，显示配置指引
  if (mode !== 'off') {
    router.push({ path: '/settings', query: { tab: 'attendance', guide: 'attendance' } })
  }
}

// 提醒弹窗和加班设置页面不需要主布局
const isPopupRoute = computed(() => {
  const name = route.name as string
  return name === 'reminder' || name === 'overtime' || name === 'translator'
})
</script>

<template>
  <router-view v-if="isPopupRoute" />
  <template v-else-if="ready">
    <OnboardingDialog v-if="showOnboarding" :show="true" @complete="onOnboardingComplete($event)" />
    <MainLayout v-else />
    <UpdateDialog />
  </template>
</template>
