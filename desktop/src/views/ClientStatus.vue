<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import ConnectionBar from '../components/ConnectionBar.vue'
import { useToast } from '../composables/useToast.js'
import {
  getAppInfo,
  getStatus,
  getTunnelRoutes,
  startTunnel,
  stopTunnel,
  disconnectAll,
  testConnectivity
} from '../tauri.js'

const router = useRouter()
const toast = useToast()

const status = ref({
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
const routes = ref([])
const appInfo = ref({ os: '', arch: '', version: '' })
const lastUpdate = ref('')

const testing = ref(false)
const testResult = ref(null)
const tunnelLoading = ref(false)
const disconnecting = ref(false)

let pollTimer = null

// 将后端返回的中文连接状态映射为 StatusBadge / ConnectionBar 使用的状态码
function mapConnStatus(s) {
  switch (s) {
    case '已连接':
      return 'connected'
    case '连接中':
    case '重连中':
      return 'connecting'
    case '连接失败':
      return 'offline'
    case '已断开':
    default:
      return 'disconnected'
  }
}

// 状态颜色（直接展示后端返回的中文文案）
function statusColor(s) {
  if (['已连接', '已认证', '运行中'].includes(s)) return 'status-green'
  if (['连接中', '重连中'].includes(s)) return 'status-yellow'
  if (['连接失败', '错误'].includes(s)) return 'status-red'
  return 'status-gray'
}

const connBadgeStatus = computed(() => mapConnStatus(status.value.connection_status))
const isConnected = computed(() => status.value.connection_status === '已连接')
const tunnelRunning = computed(() => status.value.tunnel_status === '运行中')

const modeLabel = computed(() => {
  switch (status.value.mode) {
    case 'client':
      return '客户端'
    case 'operator':
      return '实施端'
    default:
      return '未启动'
  }
})

async function refreshStatus() {
  try {
    status.value = await getStatus()
    lastUpdate.value = new Date().toLocaleString('zh-CN')
  } catch (e) {
    console.error('获取状态失败', e)
  }
  try {
    routes.value = await getTunnelRoutes()
  } catch (e) {
    routes.value = []
  }
}

async function handleStartTunnel() {
  if (!isConnected.value) {
    toast.warning('请先连接服务端')
    return
  }
  tunnelLoading.value = true
  try {
    const res = await startTunnel()
    routes.value = res.routes || []
    toast.success('隧道已启动' + (res.virtual_ip ? '，虚拟 IP: ' + res.virtual_ip : ''))
    await refreshStatus()
  } catch (e) {
    console.error('启动隧道失败', e)
    toast.error('启动隧道失败: ' + (e.message || e))
  } finally {
    tunnelLoading.value = false
  }
}

async function handleStopTunnel() {
  tunnelLoading.value = true
  try {
    await stopTunnel()
    routes.value = []
    toast.success('隧道已停止')
    await refreshStatus()
  } catch (e) {
    console.error('停止隧道失败', e)
    toast.error('停止隧道失败: ' + (e.message || e))
  } finally {
    tunnelLoading.value = false
  }
}

async function handleDisconnect() {
  disconnecting.value = true
  try {
    await disconnectAll()
    toast.success('已断开所有连接')
    await refreshStatus()
  } catch (e) {
    console.error('断开失败', e)
    toast.error('断开失败: ' + (e.message || e))
  } finally {
    disconnecting.value = false
  }
}

function handleConnect() {
  // 跳转到客户端主界面进行连接
  router.push('/client')
}

async function handleTestConnectivity() {
  testing.value = true
  testResult.value = null
  try {
    const addr = (status.value.server_addr || '').replace(/^https?:\/\//, '').replace(/\/$/, '')
    if (!addr) {
      toast.warning('未获取到服务端地址')
      return
    }
    const [host, portStr] = addr.split(':')
    const port = parseInt(portStr) || 8443
    testResult.value = await testConnectivity(host, port)
    if (testResult.value.success) {
      toast.success(testResult.value.message)
    } else {
      toast.error(testResult.value.message)
    }
  } catch (e) {
    testResult.value = {
      success: false,
      elapsed_ms: 0,
      message: '测试失败: ' + (e.message || e)
    }
    toast.error('测试失败: ' + (e.message || e))
  } finally {
    testing.value = false
  }
}

function exportLogs() {
  // 生成诊断信息文本
  const lines = [
    '==== NetTool 客户端诊断信息 ====',
    `导出时间: ${new Date().toLocaleString('zh-CN')}`,
    `软件版本: ${appInfo.value.version || '0.1.0'}`,
    `操作系统: ${appInfo.value.os} / ${appInfo.value.arch}`,
    `运行模式: ${modeLabel.value}`,
    `服务端地址: ${status.value.server_addr || '-'}`,
    `节点名称: ${status.value.node_name || '-'}`,
    `节点 ID: ${status.value.node_id || '-'}`,
    `虚拟 IP: ${status.value.virtual_ip || '-'}`,
    `连接状态: ${status.value.connection_status}`,
    `隧道状态: ${status.value.tunnel_status}`,
    `可访问网段数: ${status.value.accessible_segments_count}`,
    `已上报网段数: ${status.value.reported_segments_count}`,
    '',
    '---- 隧道路由 ----'
  ]
  routes.value.forEach((r, i) => {
    lines.push(`${i + 1}. ${r.mapped_cidr} -> ${r.target_node_name} (${r.real_cidr}) [${r.segment_name}]`)
  })

  const blob = new Blob([lines.join('\n')], { type: 'text/plain' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `nettool-diagnostic-${Date.now()}.txt`
  a.click()
  URL.revokeObjectURL(url)
}

async function init() {
  try {
    appInfo.value = await getAppInfo()
  } catch {
    // 非 Tauri 环境（浏览器调试），使用默认值
  }
  await refreshStatus()
  // 每 3 秒自动刷新状态
  pollTimer = setInterval(refreshStatus, 3000)
}

onUnmounted(() => {
  if (pollTimer) {
    clearInterval(pollTimer)
    pollTimer = null
  }
})

onMounted(init)
</script>

<template>
  <div>
    <!-- 连接状态栏 -->
    <ConnectionBar
      :server-addr="status.server_addr || ''"
      :status="connBadgeStatus"
      :label="status.node_name || ''"
      label-title="节点名称"
      @connect="handleConnect"
      @disconnect="handleDisconnect"
    />

    <!-- 综合状态 -->
    <div class="card">
      <div class="card-title">
        <span>综合状态</span>
        <button class="btn btn-sm" @click="refreshStatus">刷新</button>
      </div>
      <div class="desc-list">
        <div class="desc-label">运行模式</div>
        <div class="desc-value">{{ modeLabel }}</div>
        <div class="desc-label">连接状态</div>
        <div class="desc-value">
          <span class="status-badge" :class="statusColor(status.connection_status)">{{ status.connection_status || '-' }}</span>
        </div>
        <div class="desc-label">隧道状态</div>
        <div class="desc-value">
          <span class="status-badge" :class="statusColor(status.tunnel_status)">{{ status.tunnel_status || '-' }}</span>
        </div>
        <div class="desc-label">服务端地址</div>
        <div class="desc-value mono">{{ status.server_addr || '-' }}</div>
        <div class="desc-label">节点 ID</div>
        <div class="desc-value mono">{{ status.node_id || '-' }}</div>
        <div class="desc-label">节点名称</div>
        <div class="desc-value">{{ status.node_name || '-' }}</div>
        <div class="desc-label">虚拟 IP</div>
        <div class="desc-value mono">{{ status.virtual_ip || '-' }}</div>
        <div class="desc-label">可访问网段</div>
        <div class="desc-value">{{ status.accessible_segments_count }}</div>
        <div class="desc-label">已上报网段</div>
        <div class="desc-value">{{ status.reported_segments_count }}</div>
        <div class="desc-label">最后刷新</div>
        <div class="desc-value">{{ lastUpdate || '-' }}</div>
      </div>
    </div>

    <!-- 隧道控制 -->
    <div class="card">
      <div class="card-title">
        <span>隧道控制 ({{ routes.length }})</span>
        <div class="actions">
          <button
            v-if="!tunnelRunning"
            class="btn btn-success btn-sm"
            @click="handleStartTunnel"
            :disabled="tunnelLoading || !isConnected"
          >
            {{ tunnelLoading ? '处理中...' : '启动隧道' }}
          </button>
          <button
            v-else
            class="btn btn-danger btn-sm"
            @click="handleStopTunnel"
            :disabled="tunnelLoading"
          >
            {{ tunnelLoading ? '处理中...' : '停止隧道' }}
          </button>
        </div>
      </div>
      <div v-if="!isConnected" class="alert alert-warning">
        请先连接服务端后再启动隧道。
      </div>
      <div v-else-if="!routes.length" class="empty-state">
        <div class="empty-icon">⇆</div>
        <div>{{ tunnelRunning ? '隧道已启动，暂无下发路由' : '点击「启动隧道」建立隧道连接' }}</div>
      </div>
      <div v-else class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>映射网段</th>
              <th>目标节点</th>
              <th>真实网段</th>
              <th>网段名称</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(r, i) in routes" :key="i">
              <td class="mono">{{ r.mapped_cidr }}</td>
              <td>{{ r.target_node_name }}</td>
              <td class="mono">{{ r.real_cidr }}</td>
              <td>{{ r.segment_name }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- 诊断工具 -->
    <div class="card">
      <div class="card-title">
        <span>诊断工具</span>
      </div>
      <div class="diag-actions">
        <button class="btn" @click="handleTestConnectivity" :disabled="testing">
          {{ testing ? '测试中...' : '测试服务端连通性' }}
        </button>
        <button class="btn" @click="exportLogs">导出诊断日志</button>
      </div>

      <div v-if="testResult" class="test-result" :class="testResult.success ? 'test-success' : 'test-fail'">
        <span class="test-mark">{{ testResult.success ? '[OK]' : '[FAIL]' }}</span>
        <span class="test-msg">{{ testResult.message }}</span>
        <span class="test-time">({{ testResult.elapsed_ms }} ms)</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.diag-actions {
  display: flex;
  gap: 10px;
}

.test-result {
  margin-top: 14px;
  padding: 10px 14px;
  border-radius: var(--radius);
  font-size: 13px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.test-success {
  background: var(--status-green-bg);
  color: var(--status-green);
}

.test-fail {
  background: var(--status-red-bg);
  color: var(--status-red);
}

.test-mark {
  font-weight: 700;
}

.test-time {
  color: var(--text-muted);
  font-size: 12px;
}
</style>
