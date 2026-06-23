<script setup>
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import ConnectionBar from '../components/ConnectionBar.vue'
import StatusBadge from '../components/StatusBadge.vue'
import { useToast } from '../composables/useToast.js'
import {
  loadConfig,
  saveConfig,
  getStatus,
  connectClient,
  disconnectAll,
  testConnectivity,
  refreshReportedSegments
} from '../tauri.js'

const router = useRouter()
const toast = useToast()

const serverAddr = ref('127.0.0.1:8443')
const nodeName = ref('')
const remark = ref('')
const autoReconnect = ref(true)
const connectionStatus = ref('disconnected')
const nodeId = ref('')
const virtualIp = ref('')
const segments = ref([])
const loading = ref(false)
const connecting = ref(false)

// 连通性测试
const testing = ref(false)
const testResult = ref(null)

// 将后端返回的中文连接状态映射为 StatusBadge 使用的状态码
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

async function initConfig() {
  try {
    const config = await loadConfig()
    serverAddr.value = config.server_addr || '127.0.0.1:8443'
    nodeName.value = config.node_name || ''
    remark.value = config.remark || ''
    autoReconnect.value = config.auto_reconnect !== false
  } catch {
    // ignore
  }

  // 读取实际连接状态（后端连接在页面切换间保持）
  try {
    const st = await getStatus()
    connectionStatus.value = mapConnStatus(st.connection_status)
    nodeId.value = st.node_id || ''
    virtualIp.value = st.virtual_ip || ''
  } catch {
    // ignore
  }

  // 加载已上报网段
  await loadSegments()
}

async function handleConnect() {
  if (!serverAddr.value.trim()) {
    toast.warning('请输入服务端地址')
    return
  }
  if (!nodeName.value.trim()) {
    toast.warning('请输入节点名称')
    return
  }

  connecting.value = true
  connectionStatus.value = 'connecting'

  try {
    // 以客户端模式连接服务端
    const res = await connectClient(serverAddr.value, nodeName.value, remark.value || null)
    nodeId.value = res.node_id
    virtualIp.value = res.virtual_ip || ''
    connectionStatus.value = 'connected'

    // 连接成功后保存配置
    await saveConfig({
      mode: 'client',
      server_addr: serverAddr.value,
      node_name: nodeName.value,
      remark: remark.value,
      auto_reconnect: autoReconnect.value
    })

    toast.success('连接成功，节点 ID: ' + res.node_id)

    // 跳转到连接状态页
    router.push('/client/status')
  } catch (e) {
    console.error('连接失败', e)
    connectionStatus.value = 'disconnected'
    toast.error('连接失败: ' + (e.message || e))
  } finally {
    connecting.value = false
  }
}

async function handleDisconnect() {
  try {
    await disconnectAll()
    toast.success('已断开连接')
  } catch (e) {
    console.error('断开失败', e)
    toast.error('断开失败: ' + (e.message || e))
  }
  connectionStatus.value = 'disconnected'
  nodeId.value = ''
  virtualIp.value = ''
  segments.value = []
}

async function loadSegments() {
  loading.value = true
  try {
    segments.value = await refreshReportedSegments()
  } catch (e) {
    // 未连接时刷新会失败，静默处理
    segments.value = []
  } finally {
    loading.value = false
  }
}

async function handleTestConnectivity() {
  if (!serverAddr.value.trim()) {
    toast.warning('请先输入服务端地址')
    return
  }
  testing.value = true
  testResult.value = null
  try {
    const addr = serverAddr.value.replace(/^https?:\/\//, '').replace(/\/$/, '')
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

function goSegments() {
  router.push('/client/segments')
}

onMounted(initConfig)
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
          <label class="form-label">节点名称<span class="required">*</span></label>
          <input
            class="form-input"
            v-model="nodeName"
            placeholder="如 北京办公节点"
            :disabled="connectionStatus !== 'disconnected'"
          />
        </div>
      </div>
      <div class="form-row">
        <div class="form-group">
          <label class="form-label">备注</label>
          <input
            class="form-input"
            v-model="remark"
            placeholder="可选"
            :disabled="connectionStatus !== 'disconnected'"
          />
        </div>
        <div class="form-group">
          <label class="form-label">自动重连</label>
          <label class="checkbox-label">
            <input type="checkbox" v-model="autoReconnect" :disabled="connectionStatus !== 'disconnected'" />
            <span>连接断开后自动重连</span>
          </label>
        </div>
      </div>

      <!-- 连通性测试 -->
      <div class="conn-test">
        <button
          class="btn"
          @click="handleTestConnectivity"
          :disabled="testing || connectionStatus !== 'disconnected'"
        >
          {{ testing ? '测试中...' : '测试连通性' }}
        </button>
        <div v-if="testResult" class="test-result" :class="testResult.success ? 'test-success' : 'test-fail'">
          <span class="test-mark">{{ testResult.success ? '[OK]' : '[FAIL]' }}</span>
          <span class="test-msg">{{ testResult.message }}</span>
          <span class="test-time">({{ testResult.elapsed_ms }} ms)</span>
        </div>
      </div>
    </div>

    <!-- 已上报网段列表 -->
    <div class="card">
      <div class="card-title">
        <span>已上报网段 ({{ segments.length }})</span>
        <div class="actions">
          <button class="btn btn-sm" @click="loadSegments">刷新</button>
          <button class="btn btn-primary btn-sm" @click="goSegments">添加网段</button>
        </div>
      </div>
      <div v-if="loading" class="loading-state">加载中...</div>
      <div v-else-if="!segments.length" class="empty-state">
        <div class="empty-icon">▦</div>
        <div>暂无已上报网段，点击「添加网段」上报本地网段</div>
      </div>
      <div v-else class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>网段名称</th>
              <th>真实网段</th>
              <th>映射网段</th>
              <th>状态</th>
              <th>备注</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="seg in segments" :key="seg.id">
              <td>{{ seg.name }}</td>
              <td class="mono">{{ seg.real_cidr }}</td>
              <td class="mono">{{ seg.mapped_cidr || '-' }}</td>
              <td><StatusBadge :status="seg.status" type="segment" /></td>
              <td class="text-secondary">{{ seg.remark || '-' }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<style scoped>
.checkbox-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--text-primary);
  padding-top: 8px;
  cursor: pointer;
}

.checkbox-label input {
  cursor: pointer;
}

.conn-test {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
  margin-top: 4px;
}

.test-result {
  padding: 6px 12px;
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
