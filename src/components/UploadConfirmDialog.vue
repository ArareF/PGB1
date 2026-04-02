<script setup lang="ts">
import { useI18n } from 'vue-i18n'

useI18n()

const props = defineProps<{
  fileCount: number
  show?: boolean
}>()

defineEmits<{
  confirm: []
  cancel: []
}>()
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog">
    <div v-if="props.show !== false" class="dialog-overlay">
      <div class="dialog-content glass-strong">
        <p class="dialog-title">{{ $t('uploadConfirm.title') }}</p>
        <div class="dialog-body">
          <p class="dialog-text" v-html="$t('uploadConfirm.dragDetected', { count: `<strong>${fileCount}</strong>` })"></p>
          <p class="dialog-hint">{{ $t('uploadConfirm.uploadQuestion') }}</p>
        </div>
        <div class="dialog-actions">
          <button class="dialog-btn dialog-btn-primary" @click="$emit('confirm')">
            {{ $t('uploadConfirm.yesUploaded') }}
          </button>
          <button class="dialog-btn dialog-btn-secondary" @click="$emit('cancel')">
            {{ $t('common.cancel') }}
          </button>
        </div>
      </div>
    </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.dialog-content {
  min-width: 260px;
  max-width: 360px;
}

.dialog-body {
  gap: var(--spacing-2);
}

.dialog-text {
  font-size: var(--text-base);
  color: var(--text-primary);
}

.dialog-hint {
  font-size: var(--text-base);
  color: var(--text-secondary);
}
</style>
