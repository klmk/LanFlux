<script setup>
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import ConnectionBar from '../components/ConnectionBar.vue'
import StatusBadge from '../components/StatusBadge.vue'
import { loadConfig, testConnectivity, getAppInfo } from '../tauri.js'
import { setServerAddr, sendHeartbeat } from '../api.js'

const router = useRouter()

const serverAddr = ref('')
const nodeName = ref('')
const connectionStatus = ref('disconnected')
const nodeId = ref('')
const virtualIp = ref('')
const heartbeatStatus = ref('normal')
const lastHeartbeat = ref('')
const routes = ref([])
const appInfo = ref({ os: '', arch: '', version: '' })
const testing = ref(false)
const testResult = ref(null)

async function init() {
  try {
    const config = await loadConfig()
    serverAddr.value = config.server_addr || '127.0.0.1:8443'
    nodeName.value = config.node_name || ''
    if (serverAddr.value) {
      setServerAddr(serverAddr.value)
    }
  } catch {
    // ignore
  }

  try {
    appInfo.value = await getAppInfo()
  } catch {
    // ignore
  }

  // 模拟已连接状态（实际应从连接管理器获取）
  if (nodeName.value) {
    connectionStatus.value = 'connected'
    nodeId.value = 'desktop-001'
    await doHeartbeat()
  }
}

async function doHeartbeat() {
  try {
    const res = await sendHeartbeat({
      node_id: nodeId.value,
      reported_segments_count: 3
    })
    routes.value = res.routes || []
    heartbeatStatus.value = 'normal'
    lastHeartbeat.value = new Date().toLocaleString('zh-CN')
  } catch (e) {
    heartbeatStatus.value = 'error'
    console.error('心跳失败', e)
  }
}

function handleConnect() {
  connectionStatus.value = 'connected'
  nodeId.value = 'desktop-001'
  doHeartbeat()
}

function handleDisconnect() {
  connectionStatus.value = 'disconnected'
  nodeId.value = ''
  routes.value = []
  heartbeatStatus.value = 'normal'
  lastHeartbeat.value = ''
}

async function handleTestConnectivity() {
  testing.value = true
  testResult.value = null
  try {
    const addr = serverAddr.value.replace(/^https?:\/\//, '').replace(/\/$/, '')
    const [host, portStr] = addr.split(':')
    const port = parseInt(portStr) || 8443
    testResult.value = await testConnectivity(host, port)
  } catch (e) {
    testResult.value = {
      success: false,
      elapsed_ms: 0,
      message: '测试失败: ' + (e.message || e)
    }
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
    `当前模式: 客户端`,
    `服务端地址: ${serverAddr.value}`,
    `节点名称: ${nodeName.value}`,
    `节点 ID: ${nodeId.value || '-'}`,
    `虚拟 IP: ${virtualIp.value || '-'}`,
    `连接状态: ${connectionStatus.value}`,
    `心跳状态: ${heartbeatStatus.value}`,
    `最后心跳: ${lastHeartbeat.value || '-'}`,
    '',
    '---- 已下发路由 ----',
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

onMounted(init)
</script>

<template>
  <div>
    <!-- 连接状态栏 -->
    <ConnectionBar
      :server-addr="serverAddr"
      :status="connectionStatus"
      :label="nodeName"
      label-title="节点名称"
      @connect="handleConnect"
      @disconnect="handleDisconnect"
    />

    <!-- 连接详情 -->
    <div class="card">
      <div class="card-title">
        <span>连接详情</span>
        <button class="btn btn-sm" @click="doHeartbeat" :disabled="!nodeId">发送心跳</button>
      </div>
      <div class="desc-list">
        <div class="desc-label">连接状态</div>
        <div class="desc-value">
          <StatusBadge :status="connectionStatus" type="node" />
        </div>
        <div class="desc-label">服务端地址</div>
        <div class="desc-value mono">{{ serverAddr || '-' }}</div>
        <div class="desc-label">节点 ID</div>
        <div class="desc-value mono">{{ nodeId || '-' }}</div>
        <div class="desc-label">虚拟 IP</div>
        <div class="desc-value mono">{{ virtualIp || '-' }}</div>
        <div class="desc-label">心跳状态</div>
        <div class="desc-value">
          <span v-if="heartbeatStatus === 'normal'" class="status-badge status-green">正常</span>
          <span v-else-if="heartbeatStatus === 'error'" class="status-badge status-red">异常</span>
          <span v-else class="status-badge status-gray">未知</span>
        </div>
        <div class="desc-label">最后心跳</div>
        <div class="desc-value">{{ lastHeartbeat || '-' }}</div>
      </div>
    </div>

    <!-- 已下发路由 -->
    <div class="card">
      <div class="card-title">
        <span>已下发路由 ({{ routes.length }})</span>
      </div>
      <div v-if="!routes.length" class="empty-state">
        <div class="empty-icon">⇆</div>
        <div>暂无已下发路由</div>
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
