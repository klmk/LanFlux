<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { getStatus, disconnectAll } from '../tauri.js'

const props = defineProps({
  // 兼容旧用法：传入的服务端地址（getStatus 返回值优先）
  serverAddr: {
    type: String,
    default: ''
  },
  // 兼容旧用法：传入的连接状态（getStatus 返回值优先）
  status: {
    type: String,
    default: ''
  },
  // 兼容旧用法：节点名称或虚拟 IP
  label: {
    type: String,
    default: ''
  },
  // 兼容旧用法：标签标题
  labelTitle: {
    type: String,
    default: '节点名称'
  }
})

const emit = defineEmits(['connect', 'disconnect', 'status-update'])

const loading = ref(true)
const disconnecting = ref(false)
const statusData = ref({
  mode: 'idle',
  connection_status: '已断开',
  tunnel_status: '已断开',
  node_id: null,
  node_name: null,
  virtual_ip: null,
  server_addr: null,
  accessible_segments_count: 0,
  reported_segments_count: 0
})

let pollTimer = null

/* ------------------------------------------------------------------ *
 *  模式映射
 * ------------------------------------------------------------------ */
const modeMap = {
  idle: '未选择',
  client: '客户端',
  operator: '实施端',
  server: '服务端'
}

const modeLabel = computed(() => modeMap[statusData.value.mode] || statusData.value.mode || '-')

/* ------------------------------------------------------------------ *
 *  状态颜色映射（green / yellow / red / gray）
 * ------------------------------------------------------------------ */
function statusColor(status) {
  if (!status) return 'gray'
  const s = String(status)
  const sl = s.toLowerCase()
  // 绿色：已连接、已组网、connected、networked、online
  if (
    s.includes('已连接') ||
    s.includes('已组网') ||
    sl === 'connected' ||
    sl === 'networked' ||
    sl === 'online'
  ) {
    return 'green'
  }
  // 黄色：连接中、组网中、connecting
  if (s.includes('连接中') || s.includes('组网中') || sl === 'connecting') {
    return 'yellow'
  }
  // 红色：已断开、断开、disconnected、offline、idle
  if (
    s.includes('断开') ||
    sl === 'disconnected' ||
    sl === 'offline' ||
    sl === 'idle'
  ) {
    return 'red'
  }
  return 'gray'
}

/* ------------------------------------------------------------------ *
 *  计算属性：显示值（getStatus 优先，props 回退）
 * ------------------------------------------------------------------ */
const displayConnectionStatus = computed(
  () => statusData.value.connection_status || props.status || '已断开'
)

const displayTunnelStatus = computed(
  () => statusData.value.tunnel_status || '已断开'
)

const displayServerAddr = computed(
  () => statusData.value.server_addr || props.serverAddr || '-'
)

const displayVirtualIp = computed(
  () => statusData.value.virtual_ip || props.label || ''
)

const connectionColorClass = computed(() => statusColor(displayConnectionStatus.value))
const tunnelColorClass = computed(() => statusColor(displayTunnelStatus.value))

// 是否已连接（连接状态为绿色时视为已连接）
const isConnected = computed(() => connectionColorClass.value === 'green')

/* ------------------------------------------------------------------ *
 *  数据获取与轮询
 * ------------------------------------------------------------------ */
async function fetchStatus() {
  try {
    const data = await getStatus()
    statusData.value = data
    emit('status-update', data)
  } catch (e) {
    console.error('获取状态失败', e)
  } finally {
    loading.value = false
  }
}

/* ------------------------------------------------------------------ *
 *  连接 / 断开
 * ------------------------------------------------------------------ */
function handleConnect() {
  // 连接逻辑由父组件处理（需要服务端地址、节点名称等表单数据）
  emit('connect')
}

async function handleDisconnect() {
  if (disconnecting.value) return
  disconnecting.value = true
  try {
    await disconnectAll()
  } catch (e) {
    console.error('断开连接失败', e)
  } finally {
    disconnecting.value = false
    // 立即刷新状态
    await fetchStatus()
    // 通知父组件清理本地状态
    emit('disconnect')
  }
}

/* ------------------------------------------------------------------ *
 *  生命周期
 * ------------------------------------------------------------------ */
onMounted(() => {
  fetchStatus()
  // 每 3 秒自动刷新状态
  pollTimer = setInterval(fetchStatus, 3000)
})

onUnmounted(() => {
  if (pollTimer) {
    clearInterval(pollTimer)
    pollTimer = null
  }
})
</script>

<template>
  <div class="connection-bar">
    <div class="conn-info">
      <!-- 模式 -->
      <div class="conn-item">
        <span class="conn-label">模式</span>
        <span class="conn-value">{{ modeLabel }}</span>
      </div>
      <div class="conn-divider"></div>
      <!-- 服务端地址 -->
      <div class="conn-item">
        <span class="conn-label">服务端</span>
        <span class="conn-value mono">{{ displayServerAddr }}</span>
      </div>
      <div class="conn-divider"></div>
      <!-- 连接状态 -->
      <div class="conn-item">
        <span class="conn-label">连接状态</span>
        <span class="conn-value">
          <span class="status-dot" :class="'dot-' + connectionColorClass"></span>
          {{ displayConnectionStatus }}
        </span>
      </div>
      <div class="conn-divider"></div>
      <!-- 隧道状态 -->
      <div class="conn-item">
        <span class="conn-label">隧道状态</span>
        <span class="conn-value">
          <span class="status-dot" :class="'dot-' + tunnelColorClass"></span>
          {{ displayTunnelStatus }}
        </span>
      </div>
      <!-- 虚拟 IP（有值时显示） -->
      <template v-if="displayVirtualIp">
        <div class="conn-divider"></div>
        <div class="conn-item">
          <span class="conn-label">虚拟 IP</span>
          <span class="conn-value mono">{{ displayVirtualIp }}</span>
        </div>
      </template>
    </div>
    <div class="conn-actions">
      <button
        v-if="isConnected"
        class="btn btn-danger btn-sm"
        :disabled="disconnecting"
        @click="handleDisconnect"
      >
        {{ disconnecting ? '断开中...' : '断开' }}
      </button>
      <button
        v-else
        class="btn btn-primary btn-sm"
        @click="handleConnect"
      >
        连接
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
  display: flex;
  align-items: center;
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

/* 状态指示圆点 */
.status-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 6px;
  flex-shrink: 0;
}

.dot-green {
  background: var(--status-green);
  box-shadow: 0 0 4px var(--status-green);
}

.dot-yellow {
  background: var(--status-yellow);
  box-shadow: 0 0 4px var(--status-yellow);
}

.dot-red {
  background: var(--status-red);
  box-shadow: 0 0 4px var(--status-red);
}

.dot-gray {
  background: var(--status-gray);
}
</style>
