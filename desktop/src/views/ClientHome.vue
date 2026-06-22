<script setup>
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import ConnectionBar from '../components/ConnectionBar.vue'
import StatusBadge from '../components/StatusBadge.vue'
import { loadConfig, saveConfig } from '../tauri.js'
import { setServerAddr, registerNode, querySegments } from '../api.js'

const router = useRouter()

const serverAddr = ref('127.0.0.1:8443')
const nodeName = ref('')
const remark = ref('')
const autoReconnect = ref(true)
const connectionStatus = ref('disconnected')
const nodeId = ref('')
const segments = ref([])
const loading = ref(false)
const connecting = ref(false)

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
}

async function handleConnect() {
  if (!serverAddr.value.trim()) {
    alert('请输入服务端地址')
    return
  }
  if (!nodeName.value.trim()) {
    alert('请输入节点名称')
    return
  }

  connecting.value = true
  connectionStatus.value = 'connecting'

  try {
    // 保存配置
    await saveConfig({
      mode: 'client',
      server_addr: serverAddr.value,
      node_name: nodeName.value,
      remark: remark.value,
      auto_reconnect: autoReconnect.value
    })

    // 设置 API 服务端地址
    setServerAddr(serverAddr.value)

    // 注册节点
    const res = await registerNode({
      name: nodeName.value,
      role: 'client',
      os_type: detectOs(),
      remark: remark.value || null
    })
    nodeId.value = res.node_id

    connectionStatus.value = 'connected'

    // 加载已上报网段
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
  segments.value = []
}

async function loadSegments() {
  if (!nodeId.value) return
  loading.value = true
  try {
    segments.value = await querySegments(nodeId.value)
  } catch (e) {
    console.error('加载网段失败', e)
  } finally {
    loading.value = false
  }
}

function goSegments() {
  router.push('/client/segments')
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
    </div>

    <!-- 已上报网段列表 -->
    <div class="card">
      <div class="card-title">
        <span>已上报网段 ({{ segments.length }})</span>
        <div class="actions">
          <button class="btn btn-sm" @click="loadSegments" :disabled="!nodeId">刷新</button>
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
</style>
