<script setup lang="ts">
import type { AppSettings } from '../../composables/useSettings'

const settings = defineModel<AppSettings>({ required: true })
</script>

<template>
  <div class="settings-section">
    <h2 class="section-title">{{ $t('settings.translationTitle') }}</h2>

    <div class="form-group">
      <label class="form-label">{{ $t('settings.apiKey') }}</label>
      <input v-model="settings.translation.apiKey" type="password" class="form-input" :placeholder="$t('settings.apiKeyPlaceholder')" />
    </div>

    <div class="form-group">
      <label class="form-label">{{ $t('settings.aiModel') }}</label>
      <input v-model="settings.translation.model" list="gemini-models" class="form-input" :placeholder="$t('settings.modelPlaceholder')" />
      <datalist id="gemini-models">
        <option value="gemini-2.5-flash">Gemini 2.5 Flash ({{ $t('settings.modelFlashRecommended') }})</option>
        <option value="gemini-2.5-flash-lite">Gemini 2.5 Flash Lite ({{ $t('settings.modelFlashLiteFaster') }})</option>
        <option value="gemini-3-flash-preview">Gemini 3 Flash ({{ $t('settings.modelFlashPreview') }})</option>
        <option value="gemini-2.5-pro">Gemini 2.5 Pro ({{ $t('settings.modelProStrongest') }})</option>
      </datalist>
    </div>

    <div class="form-group">
      <label class="form-label">{{ $t('settings.globalShortcut') }}</label>
      <input v-model="settings.translation.shortcut" type="text" class="form-input" :placeholder="$t('settings.shortcutPlaceholder')" />
    </div>

    <div class="form-group">
      <label class="checkbox-label">
        <input type="checkbox" v-model="settings.translation.useCalculatorKey" />
        {{ $t('settings.interceptCalcKey') }}
      </label>
    </div>

    <div class="form-group">
      <label class="form-label">{{ $t('settings.defaultLangPair') }}</label>
      <div class="lang-pair-row">
        <select v-model="settings.translation.langA" class="form-select lang-select">
          <option value="zh-CN">{{ $t('settings.langChinese') }}</option>
          <option value="en">English</option>
          <option value="ja">日本語</option>
        </select>
        <span class="lang-separator">↔</span>
        <select v-model="settings.translation.langB" class="form-select lang-select">
          <option value="zh-CN">{{ $t('settings.langChinese') }}</option>
          <option value="en">English</option>
          <option value="ja">日本語</option>
        </select>
      </div>
    </div>
  </div>
</template>
