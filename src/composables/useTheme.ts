import { ref } from 'vue'

type Theme = 'light' | 'dark'

/* 全局响应式状态，跨组件共享 */
const currentTheme = ref<Theme>('light')

export function useTheme() {
  function initTheme() {
    const saved = localStorage.getItem('pgb1-theme') as Theme | null
    currentTheme.value = saved || 'light'
    applyTheme(currentTheme.value)
  }

  function toggleTheme() {
    currentTheme.value = currentTheme.value === 'light' ? 'dark' : 'light'
    applyTheme(currentTheme.value)
    localStorage.setItem('pgb1-theme', currentTheme.value)
  }

  function applyTheme(theme: Theme) {
    document.documentElement.setAttribute('data-theme', theme)
  }

  return {
    theme: currentTheme,
    initTheme,
    toggleTheme,
  }
}
