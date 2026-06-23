<script setup>
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import ConnectionBar from '../components/ConnectionBar.vue'
import StatusBadge from '../components/StatusBadge.vue'
import { useToast } from '../composables/useToast.js'
import {
  connectOperator,
  startTunnel,
  stopTunnel,
  getStatus,
  getAccessibleSegments,
  disconnectAll,
  testConnectivity,
  saveConfig,
  loadConfig
} from '../tauri.js'

const router = useRouter()
const { showToast } = useToast()

const serverAddr = ref('127.0.0.1:8443')
const nodeName = ref('')
const virtualIp = ref('')
// disconnected | connecting | connected
const connectionStatus = ref('disconnected')
// disconnected | connecting | connected
const tunnelStatus = ref('disconnected')
const nodeId = ref('')
const segments = ref([])
// getStatus() 原始返回，用于展示附加信息
const statusInfo = ref(null)

const loading = ref(false)
const connecting = ref(false)
const startingTunnel = ref(false)
const testing = ref(false)
const testResult = ref(null)

async function initConfig() {
  try {
    const config = await loadConfig()
    serverAddr.value = config.server_addr || '127.0.0.1:8443'
    nodeName.value = config.node_name || ''
  } catch {
    // ignore
  }
  // 尝试恢复已有连接 / 隧道状态
  await refreshStatus()
}

async function refreshStatus() {
  try {
    const s = await getStatus()
    statusInfo.value = s
    // 依据 virtual_ip 判断是否已连接
    if (s.virtual_ip) {
      virtualIp.value = s.virtual_ip
      if (s.node_id) nodeId.value = s.node_id
      if (connectionStatus.value === 'disconnected') {
        connectionStatus.value = 'connected'
      }
    }
    // 隧道状态映射
    const ts = String(s.tunnel_status || '').toLowerCase()
    if (ts.includes('connect') && !ts.includes('dis') && !ts.includes('断')) {
      tunnelStatus.value = 'connected'
    } else if (ts.includes('connecting') || ts.includes('连接中')) {
      tunnelStatus.value = 'connecting'
    } else {
      tunnelStatus.value = 'disconnected'
    }
  } catch {
    // ignore
  }
}

async function handleConnect() {
  if (!serverAddr.value.trim()) {
    showToast('请输入服务端地址', 'warning')
    return
  }
  const name = nodeName.value.trim() || '实施端-' + Math.random().toString(36).slice(2, 8)

  connecting.value = true
  connectionStatus.value = 'connecting'

  try {
    await saveConfig({
      mode: 'operator',
      server_addr: serverAddr.value,
      node_name: name,
      remark: '',
      auto_reconnect: true
    })

    // 以实施端模式连接服务端
    const res = await connectOperator(serverAddr.value, name)
    nodeId.value = res.node_id || ''
    virtualIp.value = res.virtual_ip || ''
    segments.value = res.accessible_segments || []

    connectionStatus.value = 'connected'
    showToast(
      `连接成功，虚拟 IP：${virtualIp.value || '-'}，可访问网段 ${segments.value.length} 个`,
      'success'
    )

    await refreshStatus()
  } catch (e) {
    console.error('连接失败', e)
    connectionStatus.value = 'disconnected'
    showToast('连接失败：' + (e.message || e), 'error')
  } finally {
    connecting.value = false
  }
}

async function handleDisconnect() {
  try {
    await disconnectAll()
    showToast('已断开所有连接', 'success')
  } catch (e) {
    showToast('断开失败：' + (e.message || e), 'error')
  }
  connectionStatus.value = 'disconnected'
  tunnelStatus.value = 'disconnected'
  nodeId.value = ''
  virtualIp.value = ''
  segments.value = []
  testResult.value = null
  await refreshStatus()
}

async function loadSegments() {
  if (connectionStatus.value !== 'connected') {
    showToast('请先连接服务端', 'warning')
    return
  }
  loading.value = true
  try {
    segments.value = await getAccessibleSegments()
    showToast(`已加载 ${segments.value.length} 个可访问网段`, 'info')
  } catch (e) {
    console.error('加载可访问网段失败', e)
    showToast('加载可访问网段失败：' + (e.message || e), 'error')
  } finally {
    loading.value = false
  }
}

async function handleStartTunnel() {
  startingTunnel.value = true
  tunnelStatus.value = 'connecting'
  try {
    const res = await startTunnel()
    tunnelStatus.value = 'connected'
    const routeCount = res && Array.isArray(res.routes) ? res.routes.length : 0
    showToast(`隧道启动成功，下发 ${routeCount} 条路由`, 'success')
    await refreshStatus()
  } catch (e) {
    console.error('启动隧道失败', e)
    tunnelStatus.value = 'disconnected'
    showToast('启动隧道失败：' + (e.message || e), 'error')
  } finally {
    startingTunnel.value = false
  }
}

async function handleStopTunnel() {
  try {
    await stopTunnel()
    tunnelStatus.value = 'disconnected'
    showToast('隧道已停止', 'success')
    await refreshStatus()
  } catch (e) {
    showToast('停止隧道失败：' + (e.message || e), 'error')
  }
}

async function handleTestConnectivity() {
  if (!serverAddr.value.trim()) {
    showToast('请先填写服务端地址', 'warning')
    return
  }
  testing.value = true
  testResult.value = null
  try {
    const addr = serverAddr.value.trim().replace(/^https?:\/\//, '').replace(/\/$/, '')
    const [host, portStr] = addr.split(':')
    const port = parseInt(portStr) || 8443
    testResult.value = await testConnectivity(host, port)
    if (testResult.value.success) {
      showToast('服务端连通性测试成功', 'success')
    } else {
      showToast('服务端连通性测试失败', 'error')
    }
  } catch (e) {
    testResult.value = {
      success: false,
      elapsed_ms: 0,
      message: '测试失败：' + (e.message || e)
    }
    showToast('连通性测试异常：' + (e.message || e), 'error')
  } finally {
    testing.value = false
  }
}

function goTest() {
  router.push('/operator/test')
}

function goSegments() {
  router.push('/operator/segments')
}

onMounted(initConfig)
</script>

<template>
  <div>
    <!-- 连接状态栏 -->
    <ConnectionBar
      :server-addr="serverAddr"
      :status="connectionStatus"
      :label="virtualIp"
      label-title="虚拟 IP"
      @connect="handleConnect"
      @disconnect="handleDisconnect"
    />

    <!-- 连接配置区 -->
    <div class="card">
      <div class="card-title">
        <span>连接配置</span>
      </div>
      <div class="form-row">
        <div class="form-group">
          <label class="form-label">服务端地址<span class="required">*</span></label>
          <input
            class="form-input"
            v-model="serverAddr"
            placeholder="如 192.168.1.10:8443"
            :disabled="connectionStatus !== 'disconnected'"
          />
        </div>
        <div class="form-group">
          <label class="form-label">节点名称</label>
          <input
            class="form-input"
            v-model="nodeName"
            placeholder="可选，留空将自动生成"
            :disabled="connectionStatus !== 'disconnected'"
          />
        </div>
        <div class="form-group">
          <label class="form-label">虚拟 IP</label>
          <input
            class="form-input mono"
            :value="virtualIp || '-'"
            disabled
          />
        </div>
      </div>
      <div class="form-hint">
        实施端连接后将自动分配虚拟 IP，可访问服务端授权的网段；连接成功后可启动隧道。
      </div>
    </div>

    <!-- 隧道与状态 -->
    <div class="card">
      <div class="card-title">
        <span>隧道与状态</span>
        <div class="actions">
          <button class="btn btn-sm" @click="refreshStatus">刷新状态</button>
        </div>
      </div>
      <div class="desc-list">
        <div class="desc-label">连接状态</div>
        <div class="desc-value">
          <StatusBadge :status="connectionStatus" type="node" />
        </div>
        <div class="desc-label">隧道状态</div>
        <div class="desc-value">
          <StatusBadge :status="tunnelStatus" type="node" />
        </div>
        <div class="desc-label">节点 ID</div>
        <div class="desc-value mono">{{ nodeId || (statusInfo?.node_id || '-') }}</div>
        <div class="desc-label">节点名称</div>
        <div class="desc-value">{{ statusInfo?.node_name || nodeName || '-' }}</div>
        <div class="desc-label">虚拟 IP</div>
        <div class="desc-value mono">{{ virtualIp || (statusInfo?.virtual_ip || '-') }}</div>
        <div class="desc-label">可访问网段数</div>
        <div class="desc-value">{{ statusInfo?.accessible_segments_count ?? segments.length }}</div>
      </div>

      <div class="tunnel-actions">
        <button
          class="btn btn-primary"
          @click="handleStartTunnel"
          :disabled="connectionStatus !== 'connected' || tunnelStatus === 'connected' || startingTunnel"
        >
          {{ startingTunnel ? '启动中...' : '启动隧道' }}
        </button>
        <button
          class="btn btn-danger"
          @click="handleStopTunnel"
          :disabled="tunnelStatus !== 'connected'"
        >
          停止隧道
        </button>
        <button class="btn" @click="handleTestConnectivity" :disabled="testing">
          {{ testing ? '测试中...' : '测试服务端连通性' }}
        </button>
      </div>

      <div v-if="testResult" class="test-result" :class="testResult.success ? 'test-success' : 'test-fail'">
        <span class="test-mark">{{ testResult.success ? '[OK]' : '[FAIL]' }}</span>
        <span class="test-msg">{{ testResult.message }}</span>
        <span class="test-time">({{ testResult.elapsed_ms }} ms)</span>
      </div>
    </div>

    <!-- 可访问网段列表 -->
    <div class="card">
      <div class="card-title">
        <span>可访问网段 ({{ segments.length }})</span>
        <div class="actions">
          <button class="btn btn-sm" @click="loadSegments" :disabled="connectionStatus !== 'connected'">刷新</button>
          <button class="btn btn-sm" @click="goSegments">查看全部</button>
        </div>
      </div>
      <div v-if="loading" class="loading-state">加载中...</div>
      <div v-else-if="!segments.length" class="empty-state">
        <div class="empty-icon">▦</div>
        <div>暂无可访问网段</div>
      </div>
      <div v-else class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>客户/节点</th>
              <th>网段名称</th>
              <th>真实网段</th>
              <th>映射网段</th>
              <th>状态</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(seg, i) in segments" :key="i">
              <td>{{ seg.target_node_name || seg.node_name }}</td>
              <td>{{ seg.segment_name }}</td>
              <td class="mono">{{ seg.real_cidr }}</td>
              <td class="mono">{{ seg.mapped_cidr }}</td>
              <td><StatusBadge status="active" type="segment" /></td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- 快捷操作 -->
    <div class="card">
      <div class="card-title">快捷操作</div>
      <div class="quick-actions">
        <button class="quick-btn" @click="goTest">
          <span class="quick-icon">⇄</span>
          <span class="quick-label">连通性测试</span>
          <span class="quick-desc">Ping / TCP 测试</span>
        </button>
        <button class="quick-btn" @click="goTest">
          <span class="quick-icon">▦</span>
          <span class="quick-label">IP 换算</span>
          <span class="quick-desc">真实 IP 与映射 IP 互转</span>
        </button>
        <button class="quick-btn" @click="goSegments">
          <span class="quick-icon">◉</span>
          <span class="quick-label">网段测试</span>
          <span class="quick-desc">对可访问网段进行测试</span>
        </button>
      </div>
    </div>

  </div>
</template>

<style scoped>
.tunnel-actions {
  display: flex;
  gap: 10px;
  margin-top: 16px;
  flex-wrap: wrap;
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

.quick-actions {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 14px;
}

.quick-btn {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 20px 16px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius);
  background: #fff;
  cursor: pointer;
  transition: all 0.15s;
  text-align: center;
}

.quick-btn:hover {
  border-color: var(--color-primary);
  background: var(--color-primary-light);
}

.quick-icon {
  font-size: 28px;
  color: var(--color-primary);
}

.quick-label {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.quick-desc {
  font-size: 12px;
  color: var(--text-muted);
}
</style>
