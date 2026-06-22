<script setup>
import StatusBadge from './StatusBadge.vue'

const props = defineProps({
  // 服务端地址
  serverAddr: {
    type: String,
    default: ''
  },
  // 连接状态
  status: {
    type: String,
    default: 'disconnected'
  },
  // 节点名称或虚拟 IP
  label: {
    type: String,
    default: ''
  },
  // 标签类型：node 或 segment
  labelTitle: {
    type: String,
    default: '节点名称'
  }
})

const emit = defineEmits(['connect', 'disconnect'])

function handleConnect() {
  emit('connect')
}

function handleDisconnect() {
  emit('disconnect')
}
</script>

<template>
  <div class="connection-bar">
    <div class="conn-info">
      <div class="conn-item">
        <span class="conn-label">服务端</span>
        <span class="conn-value mono">{{ serverAddr || '-' }}</span>
      </div>
      <div class="conn-divider"></div>
      <div class="conn-item">
        <span class="conn-label">状态</span>
        <StatusBadge :status="status" type="node" />
      </div>
      <div class="conn-divider"></div>
      <div class="conn-item">
        <span class="conn-label">{{ labelTitle }}</span>
        <span class="conn-value">{{ label || '-' }}</span>
      </div>
    </div>
    <div class="conn-actions">
      <button
        v-if="status === 'disconnected' || status === 'offline'"
        class="btn btn-primary btn-sm"
        @click="handleConnect"
      >
        连接
      </button>
      <button
        v-else
        class="btn btn-danger btn-sm"
        @click="handleDisconnect"
      >
        断开
      </button>
    </div>
  </div>
</template>

<style scoped>
.connection-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: #fff;
  border: 1px solid var(--border-color);
  border-radius: var(--radius);
  padding: 12px 16px;
  margin-bottom: 16px;
  box-shadow: var(--shadow);
}

.conn-info {
  display: flex;
  align-items: center;
  gap: 16px;
}

.conn-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.conn-label {
  font-size: 11px;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.conn-value {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-primary);
}

.conn-divider {
  width: 1px;
  height: 32px;
  background: var(--border-color);
}

.conn-actions {
  display: flex;
  gap: 8px;
}
</style>
