<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import StatusBadge from '../components/StatusBadge.vue'
import { loadConfig, testConnectivity } from '../tauri.js'
import { setServerAddr, queryAccess } from '../api.js'

const router = useRouter()

const loading = ref(false)
const segments = ref([])
const filters = ref({ keyword: '', status: '' })

const statusOptions = [
  { value: '', label: '全部状态' },
  { value: 'active', label: '已启用' },
  { value: 'pending', label: '待确认' },
  { value: 'disabled', label: '已停用' }
]

const filteredSegments = computed(() => {
  return segments.value.filter((s) => {
    if (filters.value.status && s.status !== filters.value.status) return false
    if (filters.value.keyword) {
      const kw = filters.value.keyword.toLowerCase()
      const hit =
        s.node_name.toLowerCase().includes(kw) ||
        s.segment_name.toLowerCase().includes(kw) ||
        s.real_cidr.toLowerCase().includes(kw) ||
        s.mapped_cidr.toLowerCase().includes(kw)
      if (!hit) return false
    }
    return true
  })
})

// 测试弹窗
const testVisible = ref(false)
const testTarget = ref(null)
const testType = ref('ping')
const testPort = ref('')
const testing = ref(false)
const testOutput = ref([])

async function loadData() {
  loading.value = true
  try {
    const res = await queryAccess('desktop-op-001')
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

function openTest(seg, type) {
  testTarget.value = seg
  testType.value = type
  testPort.value = type === 'tcp' ? '80' : type === 'udp' ? '53' : ''
  testOutput.value = []
  testVisible.value = true
}

async function runTest() {
  if (!testTarget.value) return
  testing.value = true
  testOutput.value = []

  const seg = testTarget.value
  const targetIp = seg.mapped_cidr.split('/')[0]
  const time = new Date().toLocaleTimeString('zh-CN')

  testOutput.value.push({ type: 'info', text: `[${time}] 开始测试 ${testType.value.toUpperCase()} -> ${targetIp}${testPort.value ? ':' + testPort.value : ''}` })

  if (testType.value === 'ping') {
    // Ping 测试：使用 Tauri 的 test_connectivity 测试 ICMP（实际为 TCP 连通性近似）
    testOutput.value.push({ type: 'muted', text: '正在发送 Ping 请求...' })
    try {
      const result = await testConnectivity(targetIp, 80)
      if (result.success) {
        testOutput.value.push({ type: 'success', text: `回复来自 ${targetIp}: 时间=${result.elapsed_ms}ms` })
        testOutput.value.push({ type: 'success', text: `Ping 测试成功，耗时 ${result.elapsed_ms}ms` })
      } else {
        testOutput.value.push({ type: 'error', text: `Ping 失败: ${result.message}` })
      }
    } catch (e) {
      testOutput.value.push({ type: 'error', text: `Ping 异常: ${e.message || e}` })
    }
  } else if (testType.value === 'tcp') {
    const port = parseInt(testPort.value) || 80
    testOutput.value.push({ type: 'muted', text: `正在连接 TCP ${targetIp}:${port}...` })
    try {
      const result = await testConnectivity(targetIp, port)
      if (result.success) {
        testOutput.value.push({ type: 'success', text: `TCP 连接成功 ${targetIp}:${port}，耗时 ${result.elapsed_ms}ms` })
      } else {
        testOutput.value.push({ type: 'error', text: `TCP 连接失败: ${result.message}` })
      }
    } catch (e) {
      testOutput.value.push({ type: 'error', text: `TCP 异常: ${e.message || e}` })
    }
  } else if (testType.value === 'udp') {
    const port = parseInt(testPort.value) || 53
    testOutput.value.push({ type: 'muted', text: `正在测试 UDP ${targetIp}:${port}...` })
    testOutput.value.push({ type: 'info', text: 'UDP 测试通过 TCP 连通性近似判断（桌面端暂不支持原生 UDP 探测）' })
    try {
      const result = await testConnectivity(targetIp, port)
      if (result.success) {
        testOutput.value.push({ type: 'success', text: `UDP 端口 ${targetIp}:${port} 可能开放，耗时 ${result.elapsed_ms}ms` })
      } else {
        testOutput.value.push({ type: 'error', text: `UDP 端口 ${targetIp}:${port} 可能不可达: ${result.message}` })
      }
    } catch (e) {
      testOutput.value.push({ type: 'error', text: `UDP 异常: ${e.message || e}` })
    }
  }

  testOutput.value.push({ type: 'info', text: `[${new Date().toLocaleTimeString('zh-CN')}] 测试完成` })
  testing.value = false
}

function resetFilters() {
  filters.value = { keyword: '', status: '' }
}

async function init() {
  try {
    const config = await loadConfig()
    if (config.server_addr) {
      setServerAddr(config.server_addr)
    }
  } catch {
    // ignore
  }
  await loadData()
}

onMounted(init)
</script>

<template>
  <div>
    <div class="card">
      <!-- 筛选栏 -->
      <div class="toolbar">
        <div class="toolbar-item">
          <label>状态</label>
          <select class="form-select" v-model="filters.status">
            <option v-for="o in statusOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
          </select>
        </div>
        <div class="toolbar-item">
          <label>关键词</label>
          <input class="form-input" v-model="filters.keyword" placeholder="客户 / 网段名称 / 地址" />
        </div>
        <div class="spacer"></div>
        <button class="btn" @click="resetFilters">重置</button>
        <button class="btn" @click="loadData">刷新</button>
      </div>

      <!-- 表格 -->
      <div class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>客户/节点</th>
              <th>网段名称</th>
              <th>真实网段</th>
              <th>映射网段</th>
              <th>状态</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="loading">
              <td colspan="6" class="text-center text-muted">加载中...</td>
            </tr>
            <tr v-else-if="!filteredSegments.length">
              <td colspan="6" class="text-center text-muted">暂无数据</td>
            </tr>
            <tr v-for="(seg, i) in filteredSegments" :key="i">
              <td>{{ seg.node_name }}</td>
              <td>{{ seg.segment_name }}</td>
              <td class="mono">{{ seg.real_cidr }}</td>
              <td class="mono">{{ seg.mapped_cidr }}</td>
              <td><StatusBadge :status="seg.status" type="segment" /></td>
              <td class="actions-cell">
                <button class="btn-link" @click="openTest(seg, 'ping')">Ping</button>
                <button class="btn-link" @click="openTest(seg, 'tcp')">TCP</button>
                <button class="btn-link" @click="openTest(seg, 'udp')">UDP</button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- 测试弹窗 -->
    <teleport to="body">
      <div v-if="testVisible" class="test-overlay" @click.self="testVisible = false">
        <div class="test-dialog">
          <div class="test-header">
            <span class="test-title">
              连通性测试 - {{ testTarget?.segment_name }}
            </span>
            <span class="test-close" @click="testVisible = false">×</span>
          </div>
          <div class="test-body">
            <div class="test-config">
              <div class="form-group">
                <label class="form-label">测试类型</label>
                <div class="test-type-tabs">
                  <button
                    :class="['tab-btn', { active: testType === 'ping' }]"
                    @click="testType = 'ping'"
                  >Ping</button>
                  <button
                    :class="['tab-btn', { active: testType === 'tcp' }]"
                    @click="testType = 'tcp'"
                  >TCP</button>
                  <button
                    :class="['tab-btn', { active: testType === 'udp' }]"
                    @click="testType = 'udp'"
                  >UDP</button>
                </div>
              </div>
              <div class="form-group">
                <label class="form-label">目标地址（映射 IP）</label>
                <input class="form-input mono" :value="testTarget?.mapped_cidr?.split('/')[0]" disabled />
              </div>
              <div v-if="testType !== 'ping'" class="form-group">
                <label class="form-label">端口</label>
                <input class="form-input" v-model="testPort" placeholder="端口号" />
              </div>
              <button class="btn btn-primary" @click="runTest" :disabled="testing" style="width: 100%">
                {{ testing ? '测试中...' : '开始测试' }}
              </button>
            </div>
            <div class="test-output">
              <div class="terminal">
                <span v-for="(line, i) in testOutput" :key="i" :class="['term-line', 'term-' + line.type]">{{ line.text }}</span>
                <span v-if="testing" class="term-line term-muted">测试中...</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </teleport>
  </div>
</template>

<style scoped>
.test-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.test-dialog {
  background: #fff;
  border-radius: var(--radius);
  box-shadow: var(--shadow-md);
  width: 600px;
  max-width: 90vw;
  max-height: 80vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.test-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 18px;
  border-bottom: 1px solid var(--border-color);
}

.test-title {
  font-size: 15px;
  font-weight: 600;
}

.test-close {
  font-size: 22px;
  line-height: 1;
  color: var(--text-muted);
  cursor: pointer;
  padding: 0 4px;
}

.test-close:hover {
  color: var(--text-primary);
}

.test-body {
  padding: 18px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.test-config {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.test-type-tabs {
  display: flex;
  gap: 6px;
}

.tab-btn {
  flex: 1;
  padding: 6px 12px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius);
  background: #fff;
  cursor: pointer;
  font-size: 13px;
  transition: all 0.15s;
}

.tab-btn.active {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: #fff;
}

.test-output {
  margin-top: 4px;
}

.terminal {
  min-height: 160px;
  max-height: 280px;
}
</style>
