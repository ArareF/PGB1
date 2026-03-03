<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useUpdater } from '../composables/useUpdater'
import { APP_VERSION } from '../config/app'

const { t } = useI18n()
const {
  updateAvailable,
  updateInfo,
  downloading,
  progress,
  installUpdate,
  skipVersion,
  dismiss,
} = useUpdater()
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog">
      <div v-if="updateAvailable" class="update-overlay" @mousedown.self.prevent="!downloading && dismiss()">
        <div class="update-dialog glass-strong">
          <!-- 标题 -->
          <div class="update-header">
            <svg class="update-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12 2v10m0 0l-3-3m3 3l3-3" stroke-linecap="round" stroke-linejoin="round"/>
              <path d="M4 14v4a2 2 0 002 2h12a2 2 0 002-2v-4" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            <h2>{{ t('update.title') }}</h2>
          </div>

          <!-- 版本信息 -->
          <div class="update-versions">
            <span class="version-current">{{ APP_VERSION }}</span>
            <svg class="version-arrow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M5 12h14m-4-4l4 4-4 4" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            <span class="version-new">{{ updateInfo?.version }}</span>
          </div>

          <!-- 更新说明 -->
          <div v-if="updateInfo?.body" class="update-body">
            <p>{{ updateInfo.body }}</p>
          </div>

          <!-- 下载进度 -->
          <div v-if="downloading" class="update-progress">
            <div class="progress-bar">
              <div class="progress-fill" :style="{ width: progress + '%' }"></div>
            </div>
            <span class="progress-text">{{ t('update.downloading') }} {{ progress }}%</span>
          </div>

          <!-- 按钮区 -->
          <div v-if="!downloading" class="update-actions">
            <button class="btn-update" @click="installUpdate">
              {{ t('update.install') }}
            </button>
            <button class="btn-secondary" @click="dismiss">
              {{ t('update.later') }}
            </button>
            <button class="btn-skip" @click="skipVersion">
              {{ t('update.skip') }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style>
.update-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-overlay);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--overlay-backdrop);
}

.update-dialog {
  width: 380px;
  padding: var(--space-xl);
  border-radius: var(--radius-xl);
  display: flex;
  flex-direction: column;
  gap: var(--space-lg);
}

.update-header {
  display: flex;
  align-items: center;
  gap: var(--space-sm);
}

.update-icon {
  width: 24px;
  height: 24px;
  color: var(--color-primary);
  flex-shrink: 0;
}

.update-header h2 {
  margin: 0;
  font-size: var(--font-size-lg);
  font-weight: var(--font-weight-semibold);
  color: var(--text-primary);
}

.update-versions {
  display: flex;
  align-items: center;
  gap: var(--space-sm);
  justify-content: center;
  padding: var(--space-md);
  border-radius: var(--radius-md);
  background: var(--bg-subtle);
}

.version-current {
  font-size: var(--font-size-sm);
  color: var(--text-tertiary);
  font-family: var(--font-family-mono);
}

.version-arrow {
  width: 18px;
  height: 18px;
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.version-new {
  font-size: var(--font-size-md);
  font-weight: var(--font-weight-semibold);
  color: var(--color-primary);
  font-family: var(--font-family-mono);
}

.update-body {
  max-height: 160px;
  overflow-y: auto;
  padding: var(--space-sm) var(--space-md);
  border-radius: var(--radius-sm);
  background: var(--bg-subtle);
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  line-height: 1.6;
  white-space: pre-line;
}

.update-progress {
  display: flex;
  flex-direction: column;
  gap: var(--space-xs);
}

.progress-bar {
  height: 6px;
  border-radius: 3px;
  background: var(--bg-subtle);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: 3px;
  background: var(--color-primary);
  transition: width var(--duration-normal) var(--ease-out);
}

.progress-text {
  font-size: var(--font-size-xs);
  color: var(--text-tertiary);
  text-align: center;
}

.update-actions {
  display: flex;
  flex-direction: column;
  gap: var(--space-sm);
}

.btn-update {
  padding: var(--space-sm) var(--space-md);
  border-radius: var(--radius-md);
  border: none;
  background: var(--color-primary);
  color: #fff;
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-medium);
  cursor: pointer;
  transition: opacity var(--duration-fast);
}

.btn-update:hover {
  opacity: 0.9;
}

.btn-secondary {
  padding: var(--space-xs) var(--space-md);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-light);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
  cursor: pointer;
  transition: background var(--duration-fast);
}

.btn-secondary:hover {
  background: var(--bg-subtle);
}

.btn-skip {
  padding: var(--space-xs);
  border: none;
  background: transparent;
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
  cursor: pointer;
  text-align: center;
}

.btn-skip:hover {
  color: var(--text-secondary);
}
</style>
