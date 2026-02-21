<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useTheme } from './composables/useTheme'
import { useScale } from './composables/useScale'
import { useSettings } from './composables/useSettings'
import MainLayout from './layouts/MainLayout.vue'

const route = useRoute()
const { initTheme } = useTheme()
const { initScale } = useScale()
const { loadSettings } = useSettings()

onMounted(async () => {
  initTheme()
  const settings = await loadSettings()
  // uiScale: 旧用户可能存了 0（自动模式），统一降级为 100%
  const scale = (settings?.general?.uiScale ?? 0) > 0 ? settings!.general.uiScale : 1
  initScale(scale)
})

// 提醒弹窗和加班设置页面不需要主布局
const isPopupRoute = computed(() => {
  const name = route.name as string
  return name === 'reminder' || name === 'overtime' || name === 'translator'
})
</script>

<template>
  <router-view v-if="isPopupRoute" />
  <MainLayout v-else />
</template>
