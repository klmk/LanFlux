<script setup>
import { useToast } from '../composables/useToast.js'

const { toasts, remove } = useToast()

const icons = {
  success: '✓',
  error: '✕',
  warning: '!',
  info: 'i'
}
</script>

<template>
  <div class="toast-container">
    <transition-group name="toast">
      <div
        v-for="t in toasts"
        :key="t.id"
        class="toast"
        :class="'toast-' + t.type"
        @click="remove(t.id)"
      >
        <span class="toast-icon">{{ icons[t.type] || 'i' }}</span>
        <span class="toast-msg">{{ t.message }}</span>
      </div>
    </transition-group>
  </div>
</template>

<style scoped>
.toast-container {
  position: fixed;
  top: 60px;
  right: 24px;
  z-index: 9999;
  display: flex;
  flex-direction: column;
  gap: 10px;
  pointer-events: none;
}

.toast {
  pointer-events: auto;
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 240px;
  max-width: 380px;
  padding: 10px 14px;
  border-radius: var(--radius);
  box-shadow: var(--shadow-md);
  font-size: 13px;
  line-height: 1.5;
  cursor: pointer;
  background: #fff;
  border-left: 4px solid var(--status-gray);
}

.toast-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  font-size: 11px;
  font-weight: 700;
  color: #fff;
  flex-shrink: 0;
}

.toast-msg {
  flex: 1;
  word-break: break-word;
}

.toast-success {
  border-left-color: var(--status-green);
}
.toast-success .toast-icon {
  background: var(--status-green);
}
.toast-success .toast-msg {
  color: var(--status-green);
}

.toast-error {
  border-left-color: var(--status-red);
}
.toast-error .toast-icon {
  background: var(--status-red);
}
.toast-error .toast-msg {
  color: var(--status-red);
}

.toast-warning {
  border-left-color: var(--status-yellow);
}
.toast-warning .toast-icon {
  background: var(--status-yellow);
}
.toast-warning .toast-msg {
  color: var(--status-yellow);
}

.toast-info {
  border-left-color: var(--color-primary);
}
.toast-info .toast-icon {
  background: var(--color-primary);
}
.toast-info .toast-msg {
  color: var(--text-primary);
}

/* 过渡动画 */
.toast-enter-active,
.toast-leave-active {
  transition: all 0.25s ease;
}

.toast-enter-from {
  opacity: 0;
  transform: translateX(40px);
}

.toast-leave-to {
  opacity: 0;
  transform: translateX(40px);
}
</style>
