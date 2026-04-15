<script setup lang="ts">
import { useUpdater } from '../../composables/useUpdater'
import { APP_NAME, APP_VERSION, APP_DEVELOPER } from '../../config/app'

const { checking, checkResult, manualCheck } = useUpdater()
</script>

<template>
  <div class="settings-section about-section">
    <h2 class="section-title">{{ $t('settings.aboutTitle') }}</h2>
    <div class="about-card">
      <div class="about-row">
        <span class="about-label">{{ $t('settings.softwareName') }}</span>
        <span class="about-value">{{ APP_NAME }}</span>
      </div>
      <div class="about-row">
        <span class="about-label">{{ $t('settings.versionLabel') }}</span>
        <span class="about-value">{{ APP_VERSION }}</span>
      </div>
      <div class="about-row">
        <span class="about-label">{{ $t('settings.developerLabel') }}</span>
        <span class="about-value">{{ APP_DEVELOPER }}</span>
      </div>
    </div>
    <button class="check-update-btn" :disabled="checking" @click="manualCheck">
      <template v-if="checking">{{ $t('update.checking') }}</template>
      <template v-else-if="checkResult === 'latest'">{{ $t('update.isLatest') }}</template>
      <template v-else-if="checkResult === 'error'">{{ $t('update.checkFailed') }}</template>
      <template v-else>{{ $t('update.checkUpdate') }}</template>
    </button>
  </div>
</template>
