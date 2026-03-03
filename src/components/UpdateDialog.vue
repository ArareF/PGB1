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
            <span class="version-new">V{{ updateInfo?.version }}</span>
          </div>

          <!-- 更新说明 -->
          <div v-if="updateInfo?.body" class="update-body">
            <p>{{ updateInfo.body }}</p>
          </div>

          <!-- 下载进度 -->
          <div v-if="downloading" class="update-progress">
            <div class="update-progress-bar">
              <div class="update-progress-fill" :style="{ width: progress + '%' }"></div>
            </div>
            <span class="update-progress-text">{{ t('update.downloading') }} {{ progress }}%</span>
          </div>

          <!-- 按钮区 -->
          <div v-if="!downloading" class="update-actions">
            <button class="update-btn-primary" @click="installUpdate">
              {{ t('update.install') }}
            </button>
            <div class="update-btn-row">
              <button class="update-btn-secondary" @click="dismiss">
                {{ t('update.later') }}
              </button>
              <button class="update-btn-skip" @click="skipVersion">
                {{ t('update.skip') }}
              </button>
            </div>
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
  width: 360px;
  padding: var(--spacing-6);
  border-radius: var(--radius-xl);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
}

.update-header {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.update-icon {
  width: 24px;
  height: 24px;
  color: var(--color-primary);
  flex-shrink: 0;
}

.update-header h2 {
  margin: 0;
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
}

.update-versions {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  justify-content: center;
  padding: var(--spacing-3);
  border-radius: var(--radius-md);
  background: var(--glass-subtle-bg);
}

.version-current {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
  font-family: var(--font-mono);
}

.version-arrow {
  width: 18px;
  height: 18px;
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.version-new {
  font-size: var(--text-base);
  font-weight: var(--font-semibold);
  color: var(--color-primary);
  font-family: var(--font-mono);
}

.update-body {
  max-height: 160px;
  overflow-y: auto;
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-sm);
  background: var(--glass-subtle-bg);
  font-size: var(--text-sm);
  color: var(--text-secondary);
  line-height: 1.6;
  white-space: pre-line;
}

.update-body p {
  margin: 0;
}

.update-progress {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

.update-progress-bar {
  height: 6px;
  border-radius: var(--radius-full);
  background: var(--glass-subtle-bg);
  overflow: hidden;
}

.update-progress-fill {
  height: 100%;
  border-radius: var(--radius-full);
  background: var(--color-primary);
  transition: width var(--duration-normal) var(--ease-out);
}

.update-progress-text {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  text-align: center;
}

.update-actions {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.update-btn-primary {
  padding: var(--spacing-2) var(--spacing-4);
  border-radius: var(--radius-button);
  border: none;
  background: var(--color-primary);
  color: #fff;
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  font-family: inherit;
  cursor: pointer;
  transition: opacity var(--duration-fast);
}

.update-btn-primary:hover {
  opacity: 0.9;
}

.update-btn-row {
  display: flex;
  gap: var(--spacing-2);
}

.update-btn-secondary {
  flex: 1;
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-button);
  border: 1px solid var(--border-light);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-family: inherit;
  cursor: pointer;
  transition: background var(--duration-fast);
}

.update-btn-secondary:hover {
  background: var(--bg-hover);
}

.update-btn-skip {
  flex: 1;
  padding: var(--spacing-2) var(--spacing-3);
  border: none;
  border-radius: var(--radius-button);
  background: transparent;
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  font-family: inherit;
  cursor: pointer;
  text-align: center;
  transition: color var(--duration-fast);
}

.update-btn-skip:hover {
  color: var(--text-secondary);
}
</style>
