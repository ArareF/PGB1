// src/composables/useScale.ts
import { ref } from 'vue'

// 模块级单例
const currentScale = ref(1)

function applyScale(scale: number) {
  currentScale.value = scale
  const appEl = document.getElementById('app')
  if (appEl) appEl.style.zoom = String(scale)
  document.body.style.zoom = String(scale)
}

export function useScale() {
  function initScale(scale = 1) {
    applyScale(scale)
  }

  function setManualScale(scale: number) {
    applyScale(scale)
  }

  return {
    currentScale,
    initScale,
    setManualScale,
  }
}
