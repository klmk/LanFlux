<script setup>
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import ConnectionBar from '../components/ConnectionBar.vue'
import StatusBadge from '../components/StatusBadge.vue'
import { loadConfig, saveConfig } from '../tauri.js'
import { setServerAddr, registerNode, queryAccess } from '../api.js'

const router = useRouter()

const serverAddr = ref('127.0.0.1:8443')
const virtualIp = ref('')
const connectionStatus = ref('disconnected')
const nodeId = ref('')
const segments = ref([])
const loading = ref(false)
const connecting = ref(false)

async function initConfig() {
  try {
    const config = await loadConfig()
    serverAddr.value = config.server_addr || '127.0.0.1:8443'
  } catch {
    // ignore
  }
}

async function handleConnect() {
  if (!serverAddr.value.trim()) {
    alert('请输入服务端地址')
    return
  }

  connecting.value = true
  connectionStatus.value = 'connecting'

  try {
    await saveConfig({
      mode: 'operator',
      server_addr: serverAddr.value,
      node_name: '',
      remark: '',
      auto_reconnect: true
    })

    setServerAddr(serverAddr.value)

    // 注册为实施端
    const res = await registerNode({
      name: '实施端-' + Math.random().toString(36).slice(2, 8),
      role: 'operator',
      os_type: detectOs(),
      remark: null
    })
    nodeId.value = res.node_id
    virtualIp.value = res.virtual_ip || ''

    connectionStatus.value = 'connected'

    // 加载可访问网段
    await loadSegments()
  } catch (e) {
    console.error('连接失败', e)
    connectionStatus.value = 'disconnected'
    alert('连接失败: ' + (e.message || e))
  } finally {
    connecting.value = false
  }
}

function handleDisconnect() {
  connectionStatus.value = 'disconnected'
  nodeId.value = ''
  virtualIp.value = ''
  segments.value = []
}

async function loadSegments() {
  if (!nodeId.value) return
  loading.value = true
  try {
    const res = await queryAccess(nodeId.value)
    segments.value = (res.allowed_segments || []).map((r) => ({
      segment_id: '',
      node_id: r.target_node_id,
      node_name: r.target_node_name,
      segment_name: r.segment_name,
      real_cidr: r.real_cidr,
      mapped_cidr: r.mapped_cidr,
      status: 'active'
    }))
  } catch (e) {
    console.error('加载可访问网段失败', e)
  } finally {
    loading.value = false
  }
}

function goTest() {
  router.push('/operator/test')
}

function goSegments() {
  router.push('/operator/segments')
}

function detectOs() {
  const ua = navigator.userAgent
  if (ua.includes('Win')) return 'windows'
  if (ua.includes('Linux')) return 'linux'
  if (ua.includes('Mac')) return 'macos'
  return 'unknown'
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
          <label class="form-label">虚拟 IP</label>
          <input
            class="form-input mono"
            :value="virtualIp || '-'"
            disabled
          />
        </div>
      </div>
      <div class="form-hint">
        实施端连接后将自动分配虚拟 IP，可访问服务端授权的网段。
      </div>
    </div>

    <!-- 可访问网段列表 -->
    <div class="card">
      <div class="card-title">
        <span>可访问网段 ({{ segments.length }})</span>
        <div class="actions">
          <button class="btn btn-sm" @click="loadSegments" :disabled="!nodeId">刷新</button>
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
              <td>{{ seg.node_name }}</td>
              <td>{{ seg.segment_name }}</td>
              <td class="mono">{{ seg.real_cidr }}</td>
              <td class="mono">{{ seg.mapped_cidr }}</td>
              <td><StatusBadge :status="seg.status" type="segment" /></td>
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
          <span class="quick-desc">Ping / TCP / UDP 测试</span>
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
