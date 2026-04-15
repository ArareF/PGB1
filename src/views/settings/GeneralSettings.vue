<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import type { AppSettings } from '../../composables/useSettings'
import { useSettings } from '../../composables/useSettings'
import { useTheme } from '../../composables/useTheme'
import { useScale } from '../../composables/useScale'

const settings = defineModel<AppSettings>({ required: true })
const emit = defineEmits<{ persisted: [] }>()

const { locale } = useI18n()
const { saveSettings, pickDir } = useSettings()
const { theme, toggleTheme } = useTheme()
const { setManualScale } = useScale()

async function browseDir() {
  const path = await pickDir('选择项目根目录')
  if (path) settings.value.general.projectRootDir = path
}

async function onScaleChange(e: Event) {
  const val = parseFloat((e.target as HTMLSelectElement).value)
  settings.value.general.uiScale = val
  setManualScale(val)
  // 缩放是运行时偏好，立即持久化，无需手动点保存
  await saveSettings(settings.value)
  emit('persisted')
}

async function onLanguageChange(e: Event) {
  const val = (e.target as HTMLSelectElement).value as 'zh-CN' | 'en'
  locale.value = val
  settings.value.general.language = val
  // 语言是运行时偏好，立即持久化
  await saveSettings(settings.value)
  emit('persisted')
}
</script>

<template>
  <div class="settings-section">
    <h2 class="section-title">{{ $t('settings.generalTitle') }}</h2>

    <div class="form-group">
      <label class="form-label">{{ $t('settings.projectRootDir') }}</label>
      <div class="path-input-group">
        <input v-model="settings.general.projectRootDir" type="text" class="form-input" :placeholder="$t('settings.projectRootDirPlaceholder')" />
        <button class="browse-btn" @click="browseDir">{{ $t('common.browse') }}</button>
      </div>
      <p class="form-hint">{{ $t('settings.projectRootDirHint') }}</p>
    </div>

    <div class="form-group">
      <label class="form-label">{{ $t('settings.language') }}</label>
      <select
        class="form-select"
        :value="settings.general.language"
        @change="onLanguageChange"
      >
        <option value="zh-CN">中文</option>
        <option value="en">English</option>
      </select>
    </div>

    <div class="form-group">
      <label class="form-label">{{ $t('settings.uiTheme') }}</label>
      <div class="theme-toggle-row">
        <span class="theme-current">{{ $t('settings.themeCurrent') }}{{ theme === 'light' ? $t('settings.themeLight') : $t('settings.themeDark') }}</span>
        <button class="browse-btn" @click="toggleTheme">{{ $t('settings.themeSwitchTo') }}{{ theme === 'light' ? $t('settings.themeDark') : $t('settings.themeLight') }}</button>
      </div>
    </div>

    <div class="form-group">
      <label class="form-label">{{ $t('settings.defaultFps') }}</label>
      <div class="fps-input-row">
        <input
          v-model.number="settings.preview.defaultFps"
          type="number"
          min="1"
          max="120"
          class="form-input fps-input"
        />
        <span class="fps-unit">fps</span>
      </div>
      <p class="form-hint">{{ $t('settings.defaultFpsHint') }}</p>
    </div>

    <div class="form-group">
      <label class="checkbox-label">
        <input type="checkbox" v-model="settings.preview.backgroundTransparent" />
        {{ $t('settings.transparentBg') }}
      </label>
      <p class="form-hint">{{ $t('settings.transparentBgHint') }}</p>
    </div>

    <div class="form-group">
      <label class="checkbox-label">
        <input type="checkbox" v-model="settings.general.autoStart" />
        {{ $t('settings.autoStart') }}
      </label>
      <p class="form-hint">{{ $t('settings.autoStartHint') }}</p>
    </div>

    <div class="form-group">
      <label class="form-label">{{ $t('settings.uiScale') }}</label>
      <select
        class="form-select"
        :value="settings.general.uiScale"
        @change="onScaleChange"
      >
        <option :value="0.75">75%</option>
        <option :value="0.80">80%</option>
        <option :value="0.90">90%</option>
        <option :value="1.0">100%</option>
        <option :value="1.1">110%</option>
        <option :value="1.2">120%</option>
        <option :value="1.5">150%</option>
      </select>
    </div>
  </div>
</template>
