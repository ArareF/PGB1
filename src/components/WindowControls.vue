<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'

const appWindow = getCurrentWindow()

async function minimize() {
  await appWindow.minimize()
}

async function toggleMaximize() {
  await appWindow.toggleMaximize()
}

async function close() {
  await appWindow.close()
}
</script>

<template>
  <div class="window-controls">
    <button
      class="window-control-btn minimize"
      title="最小化"
      @click="minimize"
    >
      <svg width="10" height="1" viewBox="0 0 10 1">
        <rect width="10" height="1" fill="currentColor" />
      </svg>
    </button>
    <button
      class="window-control-btn maximize"
      title="最大化"
      @click="toggleMaximize"
    >
      <svg width="10" height="10" viewBox="0 0 10 10">
        <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>
    <button
      class="window-control-btn close"
      title="关闭"
      @click="close"
    >
      <svg width="10" height="10" viewBox="0 0 10 10">
        <line x1="0" y1="0" x2="10" y2="10" stroke="currentColor" stroke-width="1.2" />
        <line x1="10" y1="0" x2="0" y2="10" stroke="currentColor" stroke-width="1.2" />
      </svg>
    </button>
  </div>
</template>

<style scoped>
.window-controls {
  display: flex;
  align-items: center;
  gap: var(--window-control-gap);
  /* 不参与窗口拖拽 */
  -webkit-app-region: no-drag;
}

.window-control-btn {
  width: var(--window-control-size);
  height: var(--window-control-size);
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: var(--window-control-bg);
  color: var(--text-secondary);
  border-radius: var(--window-control-radius);
  cursor: pointer;
  transition: var(--transition-bg);
}

.window-control-btn.minimize:hover {
  background: var(--window-minimize-hover);
}

.window-control-btn.maximize:hover {
  background: var(--window-maximize-hover);
}

.window-control-btn.close {
  background: var(--window-close-bg);
  color: var(--window-close-text);
}

.window-control-btn.close:hover {
  background: var(--window-close-hover);
  color: var(--window-close-text);
}
</style>
