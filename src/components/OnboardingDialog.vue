<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useOnboardingForm } from '../composables/useOnboardingForm'

defineProps<{ show: boolean }>()
const emit = defineEmits<{ complete: [mode: 'off' | 'auto' | 'record_only'] }>()

const { t } = useI18n()

// 表单状态机、系统扫描、保存闭环全部下沉到 useOnboardingForm composable
const {
  currentStepIndex,
  currentStep,
  isLastStep,
  STEPS,
  formLanguage,
  formProjectDir,
  formImaginePath,
  formTpCliPath,
  formTpGuiPath,
  formAttendanceMode,
  imagineAutoDetected,
  tpCliAutoDetected,
  tpGuiAutoDetected,
  canProceed,
  goNext,
  goPrev,
  setLanguage,
  selectProjectDir,
  selectImaginePath,
  selectTpCliPath,
  selectTpGuiPath,
  finish,
} = useOnboardingForm((mode) => emit('complete', mode))
</script>

<template>
  <Teleport to="body">
    <Transition name="onboarding">
      <div v-if="show" class="onboarding-overlay">
        <div class="onboarding-dialog glass-strong">
          <!-- 步骤内容 -->
          <div class="step-content">
            <!-- Step 1: 语言选择 -->
            <div v-if="currentStep === 'language'" class="step-body">
              <h2 class="step-title">{{ t('onboarding.languageTitle') }}</h2>
              <p class="step-desc">{{ t('onboarding.languageDesc') }}</p>
              <div class="language-options">
                <button
                  class="lang-btn"
                  :class="{ active: formLanguage === 'zh-CN' }"
                  @click="setLanguage('zh-CN')"
                >
                  简体中文
                </button>
                <button
                  class="lang-btn"
                  :class="{ active: formLanguage === 'en' }"
                  @click="setLanguage('en')"
                >
                  English
                </button>
              </div>
            </div>

            <!-- Step 2: 项目目录 -->
            <div v-else-if="currentStep === 'project-dir'" class="step-body">
              <h2 class="step-title">{{ t('onboarding.projectDirTitle') }}</h2>
              <p class="step-desc">{{ t('onboarding.projectDirDesc') }}</p>
              <p class="step-hint">{{ t('onboarding.projectDirHint') }}</p>
              <div class="dir-picker">
                <span class="dir-display">{{ formProjectDir || t('onboarding.notSetYet') }}</span>
                <button class="pick-btn" @click="selectProjectDir">{{ t('onboarding.selectDir') }}</button>
              </div>
            </div>

            <!-- Step 3: 工具路径 -->
            <div v-else-if="currentStep === 'tool-paths'" class="step-body">
              <h2 class="step-title">{{ t('onboarding.toolPathsTitle') }}</h2>
              <p class="step-desc">{{ t('onboarding.toolPathsDesc') }}</p>
              <div class="tool-row">
                <label class="tool-label">Imagine</label>
                <div class="tool-picker">
                  <span class="tool-display" :class="{ detected: formImaginePath }">
                    <template v-if="formImaginePath">
                      <span v-if="imagineAutoDetected" class="detect-tag">{{ t('onboarding.autoDetected') }}</span>
                      {{ formImaginePath }}
                    </template>
                    <template v-else>{{ t('onboarding.toolNotFound') }}</template>
                  </span>
                  <button class="pick-btn" @click="selectImaginePath">{{ t('common.browse') }}</button>
                </div>
              </div>
              <div class="tool-row">
                <label class="tool-label">{{ t('settings.tpCliPath') }}</label>
                <div class="tool-picker">
                  <span class="tool-display" :class="{ detected: formTpCliPath }">
                    <template v-if="formTpCliPath">
                      <span v-if="tpCliAutoDetected" class="detect-tag">{{ t('onboarding.autoDetected') }}</span>
                      {{ formTpCliPath }}
                    </template>
                    <template v-else>{{ t('onboarding.toolNotFound') }}</template>
                  </span>
                  <button class="pick-btn" @click="selectTpCliPath">{{ t('common.browse') }}</button>
                </div>
              </div>
              <div class="tool-row">
                <label class="tool-label">{{ t('settings.tpGuiPath') }}</label>
                <div class="tool-picker">
                  <span class="tool-display" :class="{ detected: formTpGuiPath }">
                    <template v-if="formTpGuiPath">
                      <span v-if="tpGuiAutoDetected" class="detect-tag">{{ t('onboarding.autoDetected') }}</span>
                      {{ formTpGuiPath }}
                    </template>
                    <template v-else>{{ t('onboarding.toolNotFound') }}</template>
                  </span>
                  <button class="pick-btn" @click="selectTpGuiPath">{{ t('common.browse') }}</button>
                </div>
              </div>
            </div>

            <!-- Step 4: 打卡模式 -->
            <div v-else-if="currentStep === 'attendance'" class="step-body">
              <h2 class="step-title">{{ t('onboarding.attendanceTitle') }}</h2>
              <p class="step-desc">{{ t('onboarding.attendanceDesc') }}</p>
              <div class="attendance-options">
                <button
                  class="mode-btn"
                  :class="{ active: formAttendanceMode === 'off' }"
                  @click="formAttendanceMode = 'off'"
                >
                  {{ t('onboarding.attendanceModeOff') }}
                </button>
                <button
                  class="mode-btn"
                  :class="{ active: formAttendanceMode === 'auto' }"
                  @click="formAttendanceMode = 'auto'"
                >
                  {{ t('onboarding.attendanceModeAuto') }}
                </button>
                <button
                  class="mode-btn"
                  :class="{ active: formAttendanceMode === 'record_only' }"
                  @click="formAttendanceMode = 'record_only'"
                >
                  {{ t('onboarding.attendanceModeRecord') }}
                </button>
              </div>
            </div>
          </div>

          <!-- 底部：圆点指示器 + 按钮 -->
          <div class="step-footer">
            <div class="step-dots">
              <span
                v-for="i in STEPS.length"
                :key="i"
                class="dot"
                :class="{ active: i - 1 === currentStepIndex, visited: i - 1 < currentStepIndex }"
              />
            </div>
            <div class="step-actions">
              <button
                v-if="currentStepIndex > 0"
                class="action-btn secondary"
                @click="goPrev"
              >
                {{ t('onboarding.prev') }}
              </button>

              <button
                v-if="!isLastStep"
                class="action-btn primary"
                :disabled="!canProceed"
                @click="goNext"
              >
                {{ t('onboarding.next') }}
              </button>

              <button
                v-if="isLastStep"
                class="action-btn primary"
                @click="finish"
              >
                {{ t('onboarding.startUsing') }}
              </button>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.onboarding-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal-backdrop);
  background: var(--overlay-backdrop);
  display: flex;
  align-items: center;
  justify-content: center;
}

.onboarding-dialog {
  width: 580px;
  max-width: 90vw;
  max-height: 85vh;
  display: flex;
  flex-direction: column;
  border-radius: var(--radius-2xl);
  overflow: hidden;
}

/* ─── 步骤内容 ───────────────────────────────────── */
.step-content {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-8) var(--spacing-8) var(--spacing-4);
}

.step-body {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-4);
  text-align: center;
}

.step-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

.step-desc {
  font-size: var(--text-base);
  color: var(--text-secondary);
  line-height: 1.6;
  max-width: 420px;
}

/* ─── 语言选择 ───────────────────────────────────── */
.language-options {
  display: flex;
  gap: var(--spacing-4);
  margin-top: var(--spacing-4);
}

.lang-btn {
  padding: var(--spacing-3) var(--spacing-8);
  border-radius: var(--radius-lg);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-base);
  cursor: pointer;
  transition: var(--transition-all);
}

.lang-btn:hover {
  border-color: var(--color-primary);
  color: var(--text-primary);
}

.lang-btn.active {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: var(--text-inverse);
}

/* ─── 目录选择 ───────────────────────────────────── */
.dir-picker,
.tool-picker {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  width: 100%;
  max-width: 420px;
}

.dir-display,
.tool-display {
  flex: 1;
  text-align: left;
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-light);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pick-btn {
  padding: var(--spacing-2) var(--spacing-4);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: var(--transition-all);
  white-space: nowrap;
  flex-shrink: 0;
}

.pick-btn:hover {
  border-color: var(--color-primary);
  color: var(--text-primary);
}

/* ─── 工具路径 ───────────────────────────────────── */
.tool-row {
  width: 100%;
  max-width: 420px;
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
  align-items: flex-start;
}

.tool-label {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

/* ─── 打卡模式 ───────────────────────────────────── */
.attendance-options {
  display: flex;
  gap: var(--spacing-3);
  margin-top: var(--spacing-4);
}

.mode-btn {
  padding: var(--spacing-3) var(--spacing-6);
  border-radius: var(--radius-lg);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: var(--transition-all);
}

.mode-btn:hover {
  border-color: var(--color-primary);
  color: var(--text-primary);
}

.mode-btn.active {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: var(--text-inverse);
}

/* ─── 提示文本 ───────────────────────────────────── */
.step-hint {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
  max-width: 420px;
  margin-top: calc(-1 * var(--spacing-2));
}

/* ─── 自动检测标签 ───────────────────────────────── */
.detect-tag {
  font-size: var(--text-xs);
  color: var(--color-success, #4caf50);
  margin-right: var(--spacing-1);
}

.tool-display.detected {
  border-color: var(--color-success, #4caf50);
}

/* ─── 底部 ───────────────────────────────────── */
.step-footer {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-4);
  padding: var(--spacing-4) var(--spacing-8) var(--spacing-6);
}

.step-dots {
  display: flex;
  gap: 6px;
}

.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--border-medium);
  transition: var(--transition-all);
}

.dot.visited {
  background: var(--color-primary);
  opacity: 0.4;
}

.dot.active {
  background: var(--color-primary);
  opacity: 1;
  width: 18px;
  border-radius: 3px;
}

.step-actions {
  display: flex;
  gap: var(--spacing-3);
  align-items: center;
}

.action-btn {
  padding: var(--spacing-2) var(--spacing-6);
  border-radius: var(--radius-lg);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: var(--transition-all);
  border: 1px solid transparent;
}

.action-btn.primary {
  background: var(--color-primary);
  color: var(--text-inverse);
  border-color: var(--color-primary);
}

.action-btn.primary:hover:not(:disabled) {
  opacity: 0.9;
}

.action-btn.primary:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.action-btn.secondary {
  background: transparent;
  color: var(--text-secondary);
  border-color: var(--border-medium);
}

.action-btn.secondary:hover {
  color: var(--text-primary);
  border-color: var(--border-heavy);
}

.action-btn.tertiary {
  background: transparent;
  color: var(--text-tertiary);
  border-color: transparent;
}

.action-btn.tertiary:hover {
  color: var(--text-secondary);
}

/* ─── 进出场动画 ───────────────────────────────────── */
.onboarding-enter-active {
  transition: opacity var(--duration-normal) var(--ease-out);
}
.onboarding-enter-active .onboarding-dialog {
  animation: dialog-enter var(--duration-normal) var(--ease-out);
}
.onboarding-leave-active {
  transition: opacity var(--duration-fast) var(--ease-in);
}
.onboarding-leave-active .onboarding-dialog {
  animation: dialog-leave var(--duration-fast) var(--ease-in);
}
.onboarding-enter-from,
.onboarding-leave-to {
  opacity: 0;
}
</style>
