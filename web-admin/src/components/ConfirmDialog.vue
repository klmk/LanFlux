<script setup>
const props = defineProps({
  // 是否显示
  visible: {
    type: Boolean,
    default: false
  },
  // 标题
  title: {
    type: String,
    default: '确认操作'
  },
  // 内容
  content: {
    type: String,
    default: ''
  },
  // 确认按钮文字
  confirmText: {
    type: String,
    default: '确认'
  },
  // 取消按钮文字
  cancelText: {
    type: String,
    default: '取消'
  },
  // 确认按钮类型：primary / danger
  confirmType: {
    type: String,
    default: 'primary'
  }
})

const emit = defineEmits(['confirm', 'cancel'])

function handleConfirm() {
  emit('confirm')
}

function handleCancel() {
  emit('cancel')
}

function handleOverlay() {
  handleCancel()
}
</script>

<template>
  <teleport to="body">
    <div v-if="visible" class="confirm-overlay" @click.self="handleOverlay">
      <div class="confirm-dialog">
        <div class="confirm-header">
          <span class="confirm-title">{{ title }}</span>
          <span class="confirm-close" @click="handleCancel">×</span>
        </div>
        <div class="confirm-body">{{ content }}</div>
        <div class="confirm-footer">
          <button class="btn" @click="handleCancel">{{ cancelText }}</button>
          <button
            class="btn"
            :class="confirmType === 'danger' ? 'btn-danger' : 'btn-primary'"
            @click="handleConfirm"
          >
            {{ confirmText }}
          </button>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.confirm-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.confirm-dialog {
  background: #fff;
  border-radius: var(--radius);
  box-shadow: var(--shadow-md);
  width: 420px;
  max-width: 90vw;
  overflow: hidden;
}

.confirm-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 18px;
  border-bottom: 1px solid var(--border-color);
}

.confirm-title {
  font-size: 15px;
  font-weight: 600;
}

.confirm-close {
  font-size: 22px;
  line-height: 1;
  color: var(--text-muted);
  cursor: pointer;
  padding: 0 4px;
}

.confirm-close:hover {
  color: var(--text-primary);
}

.confirm-body {
  padding: 20px 18px;
  font-size: 14px;
  color: var(--text-primary);
  line-height: 1.6;
  white-space: pre-wrap;
}

.confirm-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 12px 18px 16px;
}
</style>
