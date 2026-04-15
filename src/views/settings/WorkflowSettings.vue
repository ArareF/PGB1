<script setup lang="ts">
import { useSettings } from '../../composables/useSettings'
import type { AppSettings } from '../../composables/useSettings'
import { useI18n } from 'vue-i18n'

const settings = defineModel<AppSettings>({ required: true })
const { pickFile } = useSettings()
const { t } = useI18n()

async function browseFile(field: 'imaginePath' | 'texturePackerCliPath' | 'texturePackerGuiPath') {
  const path = await pickFile(t('settings.selectExecutable'), [{ name: 'Executable', extensions: ['exe'] }])
  if (path) settings.value.workflow[field] = path
}
</script>

<template>
  <div class="settings-section">
    <h2 class="section-title">{{ $t('settings.workflowTitle') }}</h2>

    <div class="form-group">
      <label class="form-label">{{ $t('settings.imaginePath') }}</label>
      <div class="path-input-group">
        <input v-model="settings.workflow.imaginePath" type="text" class="form-input" :placeholder="$t('settings.imaginePathPlaceholder')" />
        <button class="browse-btn" @click="browseFile('imaginePath')">{{ $t('common.browse') }}</button>
      </div>
      <p class="form-hint">{{ $t('settings.imaginePathHint') }}</p>
    </div>

    <div class="form-group">
      <label class="form-label">{{ $t('settings.tpCliPath') }}</label>
      <div class="path-input-group">
        <input v-model="settings.workflow.texturePackerCliPath" type="text" class="form-input" :placeholder="$t('settings.tpCliPathPlaceholder')" />
        <button class="browse-btn" @click="browseFile('texturePackerCliPath')">{{ $t('common.browse') }}</button>
      </div>
    </div>

    <div class="form-group">
      <label class="form-label">{{ $t('settings.tpGuiPath') }}</label>
      <div class="path-input-group">
        <input v-model="settings.workflow.texturePackerGuiPath" type="text" class="form-input" :placeholder="$t('settings.tpGuiPathPlaceholder')" />
        <button class="browse-btn" @click="browseFile('texturePackerGuiPath')">{{ $t('common.browse') }}</button>
      </div>
    </div>
  </div>
</template>
