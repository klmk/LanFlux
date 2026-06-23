<script setup>
import { ref, computed, onMounted } from 'vue'
import StatusBadge from '../components/StatusBadge.vue'
import { useToast } from '../composables/useToast.js'
import {
  getAccessibleSegments,
  refreshAccessibleSegments,
  pingTest,
  tcpTest
} from '../tauri.js'

const { showToast } = useToast()

const loading = ref(false)
const refreshing = ref(false)
const segments = ref([])
const filters = ref({ keyword: '' })

const filteredSegments = computed(() => {
  return segments.value.filter((s) => {
    if (filters.value.keyword) {
      const kw = filters.value.keyword.toLowerCase()
      const hit =
        (s.target_node_name || '').toLowerCase().includes(kw) ||
        (s.segment_name || '').toLowerCase().includes(kw) ||
        (s.real_cidr || '').toLowerCase().includes(kw) ||
        (s.mapped_cidr || '').toLowerCase().includes(kw)
      if (!hit) return false
    }
    return true
  })
})

// 测试弹窗
const testVisible = ref(false)
const testTarget = ref(null)
const testType = ref('ping')
const testPort = ref('80')
const testing = ref(false)
const testOutput = ref([])

// 由映射网段推导网关 IP（末位置 1）
function gatewayIp(cidr) {
  if (!cidr) return ''
  const ip = cidr.split('/')[0]
  const parts = ip.split('.')
  if (parts.length !== 4) return ip
  parts[3] = '1'
  return parts.join('.')
}

async function loadData() {
  loading.value = true
  try {
    segments.value = await getAccessibleSegments()
  } catch (e) {
    console.error('加载可访问网段失败', e)
    showToast('加载可访问网段失败：' + (e.message || e), 'error')
    segments.value = []
  } finally {
    loading.value = false
  }
}

async function handleRefresh() {
  refreshing.value = true
  try {
    segments.value = await refreshAccessibleSegments()
    showToast(`已刷新，共 ${segments.value.length} 个可访问网段`, 'success')
  } catch (e) {
    console.error('刷新可访问网段失败', e)
    showToast('刷新可访问网段失败：' + (e.message || e), 'error')
  } finally {
    refreshing.value = false
  }
}

function openTest(seg, type) {
  testTarget.value = seg
  testType.value = type
  testPort.value = type === 'tcp' ? '80' : ''
  testOutput.value = []
  testVisible.value = true
}

async function runTest() {
  if (!testTarget.value) return
  testing.value = true
  testOutput.value = []

  const seg = testTarget.value
  const targetIp = gatewayIp(seg.mapped_cidr)
  const time = new Date().toLocaleTimeString('zh-CN')

  testOutput.value.push({
    type: 'info',
    text: `[${time}] 开始 ${testType.value.toUpperCase()} 测试 -> ${targetIp}${testType.value === 'tcp' ? ':' + testPort.value : ''}`
  })

  if (testType.value === 'ping') {
    testOutput.value.push({ type: 'muted', text: '正在发送 Ping 请求...' })
    try {
      const result = await pingTest(targetIp)
      if (result.success) {
        testOutput.value.push({ type: 'success', text: `回复来自 ${result.target || targetIp}: 时间=${result.elapsed_ms}ms` })
        testOutput.value.push({ type: 'success', text: result.message })
        showToast('Ping 测试成功', 'success')
      } else {
        testOutput.value.push({ type: 'error', text: `Ping 失败: ${result.message}` })
        showToast('Ping 测试失败', 'error')
      }
    } catch (e) {
      testOutput.value.push({ type: 'error', text: `Ping 异常: ${e.message || e}` })
      showToast('Ping 异常：' + (e.message || e), 'error')
    }
  } else if (testType.value === 'tcp') {
    const port = parseInt(testPort.value) || 80
    testOutput.value.push({ type: 'muted', text: `正在连接 TCP ${targetIp}:${port}...` })
    try {
      const result = await tcpTest(targetIp, port)
      if (result.success) {
        testOutput.value.push({ type: 'success', text: `TCP 连接成功 ${result.target || targetIp}:${result.port || port}，耗时 ${result.elapsed_ms}ms` })
        testOutput.value.push({ type: 'success', text: result.message })
        showToast('TCP 测试成功', 'success')
      } else {
        testOutput.value.push({ type: 'error', text: `TCP 连接失败: ${result.message}` })
        showToast('TCP 测试失败', 'error')
      }
    } catch (e) {
      testOutput.value.push({ type: 'error', text: `TCP 异常: ${e.message || e}` })
      showToast('TCP 异常：' + (e.message || e), 'error')
    }
  }

  testOutput.value.push({ type: 'info', text: `[${new Date().toLocaleTimeString('zh-CN')}] 测试完成` })
  testing.value = false
}

function resetFilters() {
  filters.value = { keyword: '' }
}

onMounted(loadData)
</script>

<template>
  <div>
    <div class="card">
      <!-- 标题与计数 -->
      <div class="card-title">
        <span>可访问网段 ({{ segments.length }})</span>
        <div class="actions">
          <button class="btn btn-sm" @click="handleRefresh" :disabled="refreshing">
            {{ refreshing ? '刷新中...' : '刷新' }}
          </button>
        </div>
      </div>

      <!-- 筛选栏 -->
      <div class="toolbar">
        <div class="toolbar-item">
          <label>关键词</label>
          <input class="form-input" v-model="filters.keyword" placeholder="客户 / 网段名称 / 地址" />
        </div>
        <div class="spacer"></div>
        <button class="btn" @click="resetFilters">重置</button>
        <button class="btn" @click="loadData" :disabled="loading">刷新列表</button>
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
              <th>网关 IP</th>
              <th>状态</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="loading">
              <td colspan="7" class="text-center text-muted">加载中...</td>
            </tr>
            <tr v-else-if="!filteredSegments.length">
              <td colspan="7" class="text-center text-muted">暂无数据</td>
            </tr>
            <tr v-for="(seg, i) in filteredSegments" :key="i">
              <td>{{ seg.target_node_name || seg.node_name }}</td>
              <td>{{ seg.segment_name }}</td>
              <td class="mono">{{ seg.real_cidr }}</td>
              <td class="mono">{{ seg.mapped_cidr }}</td>
              <td class="mono">{{ gatewayIp(seg.mapped_cidr) }}</td>
              <td><StatusBadge status="active" type="segment" /></td>
              <td class="actions-cell">
                <button class="btn-link" @click="openTest(seg, 'ping')">Ping</button>
                <button class="btn-link" @click="openTest(seg, 'tcp')">TCP</button>
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
                </div>
              </div>
              <div class="form-group">
                <label class="form-label">目标地址（映射网关 IP）</label>
                <input class="form-input mono" :value="testTarget ? gatewayIp(testTarget.mapped_cidr) : ''" disabled />
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
                <span v-if="!testOutput.length" class="term-line term-muted">等待测试...</span>
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
