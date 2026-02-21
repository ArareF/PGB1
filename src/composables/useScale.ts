// src/composables/useScale.ts
import { ref } from 'vue'

const BASE_WIDTH = 1920
const MIN_SCALE = 0.67
const MAX_SCALE = 1.25

// 模块级单例
const manualScale = ref(0) // 0 = 自动；> 0 = 用户手动值
const currentScale = ref(1)
let resizeObserver: ResizeObserver | null = null

function calcAutoScale(width: number): number {
  const s = width / BASE_WIDTH
  return Math.min(Math.max(s, MIN_SCALE), MAX_SCALE)
}

function applyScale(scale: number) {
  currentScale.value = scale
  const appEl = document.getElementById('app')
  if (appEl) appEl.style.zoom = String(scale)
  document.body.style.zoom = String(scale)
}

export function useScale() {
  function initScale(savedManualScale = 0) {
    manualScale.value = savedManualScale

    if (!resizeObserver) {
      resizeObserver = new ResizeObserver(() => {
        const width = window.innerWidth
        if (manualScale.value > 0) {
          applyScale(manualScale.value)
        } else {
          applyScale(calcAutoScale(width))
        }
      })
      resizeObserver.observe(document.documentElement)
    }

    const width = window.innerWidth
    if (manualScale.value > 0) {
      applyScale(manualScale.value)
    } else {
      applyScale(calcAutoScale(width))
    }
  }

  function setManualScale(scale: number) {
    manualScale.value = scale
    if (scale > 0) {
      applyScale(scale)
    } else {
      applyScale(calcAutoScale(window.innerWidth))
    }
  }

  return {
    currentScale,
    manualScale,
    initScale,
    setManualScale,
  }
}
