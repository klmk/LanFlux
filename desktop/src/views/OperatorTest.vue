<script setup>
import { ref, computed } from 'vue'
import { testConnectivity } from '../tauri.js'
import { queryAccess } from '../api.js'

// ============================================================
// 连通性测试
// ============================================================

const testType = ref('ping')
const targetAddr = ref('')
const targetPort = ref('80')
const testing = ref(false)
const testOutput = ref([])

// 可访问网段列表（用于 IP 换算时选择网段映射关系）
const segments = ref([])

async function loadSegments() {
  try {
    const res = await queryAccess('desktop-op-001')
    segments.value = (res.allowed_segments || []).map((r) => ({
      real_cidr: r.real_cidr,
      mapped_cidr: r.mapped_cidr,
      segment_name: r.segment_name,
      node_name: r.target_node_name
    }))
  } catch {
    // ignore
  }
}

loadSegments()

async function runTest() {
  if (!targetAddr.value.trim()) {
    alert('请输入目标地址')
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

  if (testType.value === 'ping') {
    testOutput.value.push({ type: 'muted', text: '正在发送 Ping 请求...' })
    try {
      const result = await testConnectivity(target, 80)
      if (result.success) {
        testOutput.value.push({ type: 'success', text: `回复来自 ${target}: 时间=${result.elapsed_ms}ms` })
        testOutput.value.push({ type: 'success', text: `Ping 测试成功，耗时 ${result.elapsed_ms}ms` })
      } else {
        testOutput.value.push({ type: 'error', text: `Ping 失败: ${result.message}` })
      }
    } catch (e) {
      testOutput.value.push({ type: 'error', text: `Ping 异常: ${e.message || e}` })
    }
  } else if (testType.value === 'tcp') {
    testOutput.value.push({ type: 'muted', text: `正在连接 TCP ${target}:${port}...` })
    try {
      const result = await testConnectivity(target, port)
      if (result.success) {
        testOutput.value.push({ type: 'success', text: `TCP 连接成功 ${target}:${port}，耗时 ${result.elapsed_ms}ms` })
      } else {
        testOutput.value.push({ type: 'error', text: `TCP 连接失败: ${result.message}` })
      }
    } catch (e) {
      testOutput.value.push({ type: 'error', text: `TCP 异常: ${e.message || e}` })
    }
  } else if (testType.value === 'udp') {
    testOutput.value.push({ type: 'muted', text: `正在测试 UDP ${target}:${port}...` })
    testOutput.value.push({ type: 'info', text: '注：UDP 测试通过 TCP 连通性近似判断' })
    try {
      const result = await testConnectivity(target, port)
      if (result.success) {
        testOutput.value.push({ type: 'success', text: `UDP 端口 ${target}:${port} 可能开放，耗时 ${result.elapsed_ms}ms` })
      } else {
        testOutput.value.push({ type: 'error', text: `UDP 端口 ${target}:${port} 可能不可达: ${result.message}` })
      }
    } catch (e) {
      testOutput.value.push({ type: 'error', text: `UDP 异常: ${e.message || e}` })
    }
  }

  testOutput.value.push({ type: 'info', text: `[${new Date().toLocaleTimeString('zh-CN')}] 测试完成` })
  testing.value = false
}

function clearOutput() {
  testOutput.value = []
}

// ============================================================
// IP 换算工具
// ============================================================

const convertMode = ref('real2mapped') // real2mapped 或 mapped2real
const convertIp = ref('')
const convertCidrPair = ref('') // 选中的网段映射对
const convertResult = ref('')
const convertError = ref('')

const cidrPairOptions = computed(() => {
  return segments.value.map((s, i) => ({
    value: `${s.real_cidr}|${s.mapped_cidr}`,
    label: `${s.node_name} - ${s.segment_name} (${s.real_cidr} <-> ${s.mapped_cidr})`
  }))
})

// IP 地址转 u32（主机字节序）
function ipToU32(ip) {
  const parts = ip.split('.').map(Number)
  if (parts.length !== 4 || parts.some((p) => isNaN(p) || p < 0 || p > 255)) {
    throw new Error('无效的 IPv4 地址: ' + ip)
  }
  return ((parts[0] << 24) | (parts[1] << 16) | (parts[2] << 8) | parts[3]) >>> 0
}

// u32 转 IP 字符串
function u32ToIp(val) {
  return [(val >>> 24) & 0xff, (val >>> 16) & 0xff, (val >>> 8) & 0xff, val & 0xff].join('.')
}

// 解析 CIDR
function parseCidr(cidr) {
  const [ipStr, prefixStr] = cidr.split('/')
  if (!prefixStr) throw new Error('无效的 CIDR: ' + cidr)
  const ip = ipToU32(ipStr)
  const prefix = parseInt(prefixStr)
  if (isNaN(prefix) || prefix < 0 || prefix > 32) {
    throw new Error('无效的前缀长度: ' + prefixStr)
  }
  return { ip, prefix }
}

// 根据前缀计算掩码
function netmaskFromPrefix(prefix) {
  if (prefix === 0) return 0
  return (0xffffffff << (32 - prefix)) >>> 0
}

function doConvert() {
  convertResult.value = ''
  convertError.value = ''

  if (!convertIp.value.trim()) {
    convertError.value = '请输入 IP 地址'
    return
  }
  if (!convertCidrPair.value) {
    convertError.value = '请选择网段映射关系'
    return
  }

  try {
    const [realCidr, mappedCidr] = convertCidrPair.value.split('|')
    const { ip: realNet, prefix: realPrefix } = parseCidr(realCidr)
    const { ip: mappedNet, prefix: mappedPrefix } = parseCidr(mappedCidr)

    if (realPrefix !== mappedPrefix) {
      throw new Error(`前缀长度不匹配: 真实 ${realPrefix} vs 映射 ${mappedPrefix}`)
    }

    const mask = netmaskFromPrefix(realPrefix)
    const ip = ipToU32(convertIp.value.trim())
    const host = (ip & ~mask) >>> 0

    let resultIp
    if (convertMode.value === 'real2mapped') {
      resultIp = (mappedNet | host) >>> 0
      convertResult.value = u32ToIp(resultIp)
    } else {
      resultIp = (realNet | host) >>> 0
      convertResult.value = u32ToIp(resultIp)
    }
  } catch (e) {
    convertError.value = e.message || String(e)
  }
}
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
          <button
            :class="['tab-btn', { active: testType === 'udp' }]"
            @click="testType = 'udp'"
          >UDP 端口</button>
        </div>
      </div>

      <!-- 目标地址 -->
      <div class="form-row">
        <div class="form-group" style="flex: 2">
          <label class="form-label">目标地址（映射 IP）<span class="required">*</span></label>
          <input
            class="form-input mono"
            v-model="targetAddr"
            placeholder="如 172.16.1.10"
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

      <!-- 测试结果输出 -->
      <div class="terminal mt-16">
        <span v-if="!testOutput.length" class="term-line term-muted">等待测试...</span>
        <span v-for="(line, i) in testOutput" :key="i" :class="['term-line', 'term-' + line.type]">{{ line.text }}</span>
        <span v-if="testing" class="term-line term-muted">测试中...</span>
      </div>
    </div>

    <!-- IP 换算工具 -->
    <div class="card">
      <div class="card-title">
        <span>IP 换算工具</span>
      </div>

      <div class="alert alert-info">
        <div>
          通过网段映射关系，在真实 IP 与映射 IP 之间双向换算。
          <br />例如：真实网段 192.168.1.0/24 映射到 172.16.1.0/24，则真实 IP 192.168.1.10 对应映射 IP 172.16.1.10。
        </div>
      </div>

      <!-- 换算方向 -->
      <div class="form-group">
        <label class="form-label">换算方向</label>
        <div class="test-type-tabs">
          <button
            :class="['tab-btn', { active: convertMode === 'real2mapped' }]"
            @click="convertMode = 'real2mapped'"
          >真实 IP &rarr; 映射 IP</button>
          <button
            :class="['tab-btn', { active: convertMode === 'mapped2real' }]"
            @click="convertMode = 'mapped2real'"
          >映射 IP &rarr; 真实 IP</button>
        </div>
      </div>

      <!-- 网段映射选择 -->
      <div class="form-group">
        <label class="form-label">网段映射关系</label>
        <select class="form-select" v-model="convertCidrPair">
          <option value="">请选择...</option>
          <option v-for="o in cidrPairOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>
      </div>

      <!-- IP 输入 -->
      <div class="form-group">
        <label class="form-label">
          {{ convertMode === 'real2mapped' ? '真实 IP' : '映射 IP' }}
        </label>
        <input
          class="form-input mono"
          v-model="convertIp"
          :placeholder="convertMode === 'real2mapped' ? '如 192.168.1.10' : '如 172.16.1.10'"
        />
      </div>

      <button class="btn btn-primary" @click="doConvert">换算</button>

      <!-- 换算结果 -->
      <div v-if="convertError" class="alert alert-error mt-16">
        <span>{{ convertError }}</span>
      </div>
      <div v-if="convertResult" class="convert-result mt-16">
        <div class="result-label">
          {{ convertMode === 'real2mapped' ? '映射 IP' : '真实 IP' }}
        </div>
        <div class="result-value mono">{{ convertResult }}</div>
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

.convert-result {
  background: var(--color-primary-light);
  border: 1px solid var(--color-primary);
  border-radius: var(--radius);
  padding: 14px 18px;
}

.result-label {
  font-size: 12px;
  color: var(--color-primary);
  margin-bottom: 6px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.result-value {
  font-size: 22px;
  font-weight: 700;
  color: var(--text-primary);
}
</style>
