<script setup lang="ts">
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
    <div v-if="props.show !== false" class="dialog-overlay" @click.self="$emit('cancel')">
      <div class="dialog-content glass-strong">
        <p class="dialog-title">上传确认</p>
        <div class="dialog-body">
          <p class="dialog-text">
            检测到您拖拽了 <strong>{{ fileCount }}</strong> 个文件
          </p>
          <p class="dialog-hint">是否已成功上传到网盘？</p>
        </div>
        <div class="dialog-actions">
          <button class="dialog-btn dialog-btn-primary" @click="$emit('confirm')">
            是，已上传
          </button>
          <button class="dialog-btn dialog-btn-secondary" @click="$emit('cancel')">
            取消
          </button>
        </div>
      </div>
    </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal, 1000);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--overlay-backdrop);
  backdrop-filter: blur(var(--glass-light-blur));
}

.dialog-content {
  min-width: 260px;
  max-width: 360px;
  border-radius: var(--floating-navbar-radius);
  padding: var(--spacing-6);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-5);
}

.dialog-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
}

.dialog-body {
  display: flex;
  flex-direction: column;
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

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--spacing-3);
}

.dialog-btn {
  display: inline-flex;
  align-items: center;
  height: var(--button-height);
  padding: 0 var(--spacing-5);
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  border-radius: var(--radius-md);
  border: none;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.dialog-btn-primary {
  background: var(--color-primary);
  color: var(--color-neutral-0);
}

.dialog-btn-primary:hover {
  background: var(--color-primary-600);
}

.dialog-btn-secondary {
  background: transparent;
  color: var(--text-secondary);
  border: 1px solid var(--border-medium);
}

.dialog-btn-secondary:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}
</style>

<style>
/* 弹窗进出动画 */
/* 根元素 .dialog-overlay 必须有 transition，Vue 以此计算等待时长 */
.dialog-enter-active {
  transition: opacity var(--duration-dialog) var(--ease-out);
}
.dialog-leave-active {
  transition: opacity var(--duration-dialog) var(--ease-in);
}
.dialog-enter-from,
.dialog-leave-to {
  opacity: 0;
}
/* 内容区额外的 transform 动画 */
.dialog-enter-active .dialog-content {
  transition: transform var(--duration-dialog) var(--ease-out);
}
.dialog-leave-active .dialog-content {
  transition: transform var(--duration-dialog) var(--ease-in);
}
.dialog-enter-from .dialog-content {
  transform: translateY(16px) scale(0.97);
}
.dialog-leave-to .dialog-content {
  transform: translateY(8px) scale(0.97);
}
</style>
