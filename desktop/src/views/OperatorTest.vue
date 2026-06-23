<script setup>
import { ref, computed, onMounted } from 'vue'
import { useToast } from '../composables/useToast.js'
import { pingTest, tcpTest, convertIp, getAccessibleSegments } from '../tauri.js'

const { showToast } = useToast()

// ============================================================
// 连通性测试
// ============================================================

const testType = ref('ping')
const targetAddr = ref('')
const targetPort = ref('80')
const testing = ref(false)
const testOutput = ref([])
// 最近一次测试结果摘要
const lastResult = ref(null)
// 测试历史
const history = ref([])
let historySeq = 0

// 可访问网段列表（用于预填目标 IP）
const segments = ref([])

// 由映射网段推导网关 IP（末位置 1）
function gatewayIp(cidr) {
  if (!cidr) return ''
  const ip = cidr.split('/')[0]
  const parts = ip.split('.')
  if (parts.length !== 4) return ip
  parts[3] = '1'
  return parts.join('.')
}

// 预填选项：可访问网段的映射网关 IP
const segmentOptions = computed(() => {
  return segments.value.map((s) => {
    const ip = gatewayIp(s.mapped_cidr)
    return {
      value: ip,
      label: `${s.target_node_name || s.node_name} - ${s.segment_name} (${ip})`
    }
  })
})

async function loadSegments() {
  try {
    segments.value = await getAccessibleSegments()
    // 有可访问网段时自动预填第一个目标 IP
    if (!targetAddr.value && segmentOptions.value.length) {
      targetAddr.value = segmentOptions.value[0].value
    }
  } catch (e) {
    console.error('加载可访问网段失败', e)
  }
}

function onPrefillChange(e) {
  targetAddr.value = e.target.value
}

async function runTest() {
  if (!targetAddr.value.trim()) {
    showToast('请输入目标地址', 'warning')
    return
  }

  testing.value = true
  const time = new Date().toLocaleTimeString('zh-CN')
  const target = targetAddr.value.trim()
  const port = parseInt(targetPort.value) || 80

  testOutput.value = []
  testOutput.value.push({
    type: 'info',
    text: `[${time}] 开始 ${testType.value.toUpperCase()} 测试 -> ${target}${testType.value !== 'ping' ? ':' + port : ''}`
  })

  let success = false
  let elapsed = 0
  let message = ''

  if (testType.value === 'ping') {
    testOutput.value.push({ type: 'muted', text: '正在发送 Ping 请求...' })
    try {
      const result = await pingTest(target)
      success = !!result.success
      elapsed = result.elapsed_ms || 0
      message = result.message || ''
      if (success) {
        testOutput.value.push({ type: 'success', text: `回复来自 ${result.target || target}: 时间=${elapsed}ms` })
        testOutput.value.push({ type: 'success', text: message })
        showToast('Ping 测试成功', 'success')
      } else {
        testOutput.value.push({ type: 'error', text: `Ping 失败: ${message}` })
        showToast('Ping 测试失败', 'error')
      }
    } catch (e) {
      message = e.message || String(e)
      testOutput.value.push({ type: 'error', text: `Ping 异常: ${message}` })
      showToast('Ping 异常：' + message, 'error')
    }
  } else if (testType.value === 'tcp') {
    testOutput.value.push({ type: 'muted', text: `正在连接 TCP ${target}:${port}...` })
    try {
      const result = await tcpTest(target, port)
      success = !!result.success
      elapsed = result.elapsed_ms || 0
      message = result.message || ''
      if (success) {
        testOutput.value.push({ type: 'success', text: `TCP 连接成功 ${result.target || target}:${result.port || port}，耗时 ${elapsed}ms` })
        testOutput.value.push({ type: 'success', text: message })
        showToast('TCP 测试成功', 'success')
      } else {
        testOutput.value.push({ type: 'error', text: `TCP 连接失败: ${message}` })
        showToast('TCP 测试失败', 'error')
      }
    } catch (e) {
      message = e.message || String(e)
      testOutput.value.push({ type: 'error', text: `TCP 异常: ${message}` })
      showToast('TCP 异常：' + message, 'error')
    }
  }

  testOutput.value.push({ type: 'info', text: `[${new Date().toLocaleTimeString('zh-CN')}] 测试完成` })
  testing.value = false

  // 记录结果摘要
  lastResult.value = {
    type: testType.value,
    target,
    port: testType.value === 'ping' ? null : port,
    success,
    elapsed_ms: elapsed,
    message
  }

  // 写入历史（最新在前，最多 20 条）
  history.value.unshift({
    id: ++historySeq,
    time: new Date().toLocaleString('zh-CN'),
    ...lastResult.value
  })
  if (history.value.length > 20) {
    history.value = history.value.slice(0, 20)
  }
}

function clearOutput() {
  testOutput.value = []
  lastResult.value = null
}

function clearHistory() {
  history.value = []
  showToast('已清空测试历史', 'info')
}

// ============================================================
// IP 换算工具
// ============================================================

const convertInput = ref('')
const convertResult = ref(null)
const convertError = ref('')
const converting = ref(false)

// 换算预填选项：可访问网段（真实网关 / 映射网关）
const convertOptions = computed(() => {
  const opts = []
  segments.value.forEach((s) => {
    if (s.real_cidr) {
      opts.push({
        value: gatewayIp(s.real_cidr),
        label: `${s.target_node_name || s.node_name} - ${s.segment_name} 真实网关 (${gatewayIp(s.real_cidr)})`
      })
    }
    if (s.mapped_cidr) {
      opts.push({
        value: gatewayIp(s.mapped_cidr),
        label: `${s.target_node_name || s.node_name} - ${s.segment_name} 映射网关 (${gatewayIp(s.mapped_cidr)})`
      })
    }
  })
  return opts
})

function onConvertPrefillChange(e) {
  convertInput.value = e.target.value
}

async function doConvert() {
  convertResult.value = null
  convertError.value = ''

  if (!convertInput.value.trim()) {
    convertError.value = '请输入 IP 地址'
    return
  }

  converting.value = true
  try {
    const res = await convertIp(convertInput.value.trim())
    convertResult.value = res
    if (res.mapped_ip) {
      showToast('IP 换算成功', 'success')
    } else {
      showToast(res.message || '未匹配到网段映射', 'warning')
    }
  } catch (e) {
    convertError.value = '换算失败：' + (e.message || e)
    showToast('IP 换算失败：' + (e.message || e), 'error')
  } finally {
    converting.value = false
  }
}

onMounted(loadSegments)
</script>

<template>
  <div>
    <!-- 连通性测试 -->
    <div class="card">
      <div class="card-title">
        <span>连通性测试</span>
        <button class="btn btn-sm" @click="clearOutput">清空输出</button>
      </div>

      <!-- 测试类型选择 -->
      <div class="form-group">
        <label class="form-label">测试类型</label>
        <div class="test-type-tabs">
          <button
            :class="['tab-btn', { active: testType === 'ping' }]"
            @click="testType = 'ping'"
          >Ping (ICMP)</button>
          <button
            :class="['tab-btn', { active: testType === 'tcp' }]"
            @click="testType = 'tcp'"
          >TCP 端口</button>
        </div>
      </div>

      <!-- 从可访问网段预填 -->
      <div v-if="segmentOptions.length" class="form-group">
        <label class="form-label">从可访问网段预填目标</label>
        <select class="form-select" :value="targetAddr" @change="onPrefillChange">
          <option value="">请选择...</option>
          <option v-for="o in segmentOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>
      </div>

      <!-- 目标地址 -->
      <div class="form-row">
        <div class="form-group" style="flex: 2">
          <label class="form-label">目标地址（映射 IP）<span class="required">*</span></label>
          <input
            class="form-input mono"
            v-model="targetAddr"
            placeholder="如 100.64.1.1"
          />
        </div>
        <div v-if="testType !== 'ping'" class="form-group" style="flex: 1">
          <label class="form-label">端口</label>
          <input
            class="form-input"
            v-model="targetPort"
            placeholder="如 80"
          />
        </div>
      </div>

      <button class="btn btn-primary" @click="runTest" :disabled="testing">
        {{ testing ? '测试中...' : '开始测试' }}
      </button>

      <!-- 最近一次结果摘要 -->
      <div v-if="lastResult" class="test-result mt-16" :class="lastResult.success ? 'test-success' : 'test-fail'">
        <span class="test-mark">{{ lastResult.success ? '[OK]' : '[FAIL]' }}</span>
        <span class="test-msg">
          {{ lastResult.type.toUpperCase() }} ->
          {{ lastResult.target }}{{ lastResult.port ? ':' + lastResult.port : '' }}
        </span>
        <span class="test-time">({{ lastResult.elapsed_ms }} ms)</span>
      </div>

      <!-- 测试结果输出 -->
      <div class="terminal mt-16">
        <span v-if="!testOutput.length" class="term-line term-muted">等待测试...</span>
        <span v-for="(line, i) in testOutput" :key="i" :class="['term-line', 'term-' + line.type]">{{ line.text }}</span>
        <span v-if="testing" class="term-line term-muted">测试中...</span>
      </div>
    </div>

    <!-- 测试历史 -->
    <div class="card">
      <div class="card-title">
        <span>测试历史 ({{ history.length }})</span>
        <button class="btn btn-sm" @click="clearHistory" :disabled="!history.length">清空历史</button>
      </div>
      <div v-if="!history.length" class="empty-state">
        <div class="empty-icon">⇄</div>
        <div>暂无测试记录</div>
      </div>
      <div v-else class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>时间</th>
              <th>类型</th>
              <th>目标</th>
              <th>结果</th>
              <th>耗时</th>
              <th>详情</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="h in history" :key="h.id">
              <td class="text-muted">{{ h.time }}</td>
              <td>{{ h.type.toUpperCase() }}</td>
              <td class="mono">{{ h.target }}{{ h.port ? ':' + h.port : '' }}</td>
              <td>
                <span :class="['status-badge', h.success ? 'status-green' : 'status-red']">
                  {{ h.success ? '成功' : '失败' }}
                </span>
              </td>
              <td>{{ h.elapsed_ms }} ms</td>
              <td class="text-secondary">{{ h.message || '-' }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- IP 换算工具 -->
    <div class="card">
      <div class="card-title">
        <span>IP 换算工具</span>
      </div>

      <div class="alert alert-info">
        <div>
          输入真实 IP 或映射 IP，由后端根据可访问网段映射关系自动换算。
          <br />例如：真实网段 192.168.1.0/24 映射到 100.64.1.0/24，则真实 IP 192.168.1.10 对应映射 IP 100.64.1.10。
        </div>
      </div>

      <!-- 从可访问网段预填 -->
      <div v-if="convertOptions.length" class="form-group">
        <label class="form-label">从可访问网段预填</label>
        <select class="form-select" :value="convertInput" @change="onConvertPrefillChange">
          <option value="">请选择...</option>
          <option v-for="o in convertOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>
      </div>

      <!-- IP 输入 -->
      <div class="form-group">
        <label class="form-label">输入 IP</label>
        <input
          class="form-input mono"
          v-model="convertInput"
          placeholder="如 192.168.1.10 或 100.64.1.10"
          @keyup.enter="doConvert"
        />
      </div>

      <button class="btn btn-primary" @click="doConvert" :disabled="converting">
        {{ converting ? '换算中...' : '换算' }}
      </button>

      <!-- 换算结果 -->
      <div v-if="convertError" class="alert alert-error mt-16">
        <span>{{ convertError }}</span>
      </div>
      <div v-if="convertResult" class="convert-result mt-16">
        <div class="desc-list">
          <div class="desc-label">输入 IP</div>
          <div class="desc-value mono">{{ convertResult.input_ip || '-' }}</div>
          <div class="desc-label">真实网段</div>
          <div class="desc-value mono">{{ convertResult.real_cidr || '-' }}</div>
          <div class="desc-label">映射网段</div>
          <div class="desc-value mono">{{ convertResult.mapped_cidr || '-' }}</div>
          <div class="desc-label">换算结果</div>
          <div class="desc-value mono result-value">{{ convertResult.mapped_ip || '-' }}</div>
          <div class="desc-label">说明</div>
          <div class="desc-value">{{ convertResult.message || '-' }}</div>
        </div>
      </div>
    </div>

  </div>
</template>

<style scoped>
.test-type-tabs {
  display: flex;
  gap: 6px;
}

.tab-btn {
  flex: 1;
  padding: 7px 12px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius);
  background: #fff;
  cursor: pointer;
  font-size: 13px;
  transition: all 0.15s;
  color: var(--text-primary);
}

.tab-btn:hover {
  border-color: var(--color-primary);
  color: var(--color-primary);
}

.tab-btn.active {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: #fff;
}

.terminal {
  min-height: 180px;
  max-height: 320px;
}

.test-result {
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

.convert-result {
  background: var(--color-primary-light);
  border: 1px solid var(--color-primary);
  border-radius: var(--radius);
  padding: 14px 18px;
}

.convert-result .desc-list {
  grid-template-columns: 110px 1fr;
}

.result-value {
  font-size: 18px;
  font-weight: 700;
  color: var(--color-primary);
}
</style>
