<script setup>
import { ref, computed, onMounted } from 'vue'
import { getLogs } from '../api.js'

const loading = ref(true)
const logs = ref([])
const activeCategory = ref('all')
const levelFilter = ref('')

const categoryTabs = [
  { value: 'all', label: '全部日志' },
  { value: 'system', label: '系统日志' },
  { value: 'node', label: '节点连接' },
  { value: 'segment', label: '网段上报' },
  { value: 'route', label: '路由下发' },
  { value: 'policy', label: '权限变更' },
  { value: 'forward', label: '转发异常' }
]

const levelOptions = [
  { value: '', label: '全部级别' },
  { value: 'info', label: '信息' },
  { value: 'warning', label: '警告' },
  { value: 'error', label: '错误' }
]

const categoryLabel = {
  system: '系统日志',
  node: '节点连接',
  segment: '网段上报',
  route: '路由下发',
  policy: '权限变更',
  forward: '转发异常'
}

const levelMap = {
  info: { label: '信息', class: 'status-blue' },
  warning: { label: '警告', class: 'status-orange' },
  error: { label: '错误', class: 'status-red' }
}

const filteredLogs = computed(() => {
  return logs.value.filter((l) => {
    if (activeCategory.value !== 'all' && l.category !== activeCategory.value) return false
    if (levelFilter.value && l.level !== levelFilter.value) return false
    return true
  })
})

const stats = computed(() => ({
  total: logs.value.length,
  error: logs.value.filter((l) => l.level === 'error').length,
  warning: logs.value.filter((l) => l.level === 'warning').length
}))

// 诊断工具
const diagResult = ref(null)
const diagRunning = ref(false)

const diagTools = [
  { id: 'connectivity', label: '连通性检测', desc: '检测服务端与各实施端的连接状态' },
  { id: 'route', label: '路由一致性检查', desc: '检查已下发路由与配置是否一致' },
  { id: 'conflict', label: '网段冲突扫描', desc: '扫描是否存在真实网段地址冲突' },
  { id: 'session', label: '会话状态诊断', desc: '诊断异常会话及超时配置' }
]

async function runDiag(tool) {
  diagRunning.value = true
  diagResult.value = null
  // 模拟诊断过程
  await new Promise((r) => setTimeout(r, 600))
  const results = {
    connectivity: { ok: true, message: '连通性检测完成：3 个实施端全部可达，平均延迟 32ms。' },
    route: { ok: true, message: '路由一致性检查完成：共 8 条路由，全部与配置一致，无异常。' },
    conflict: { ok: false, message: '网段冲突扫描完成：发现 1 处冲突 — 上海监控网段（192.168.30.0/24）与北京办公网段真实地址相同。' },
    session: { ok: true, message: '会话状态诊断完成：当前 5 个活跃会话，1 个空闲会话，无异常超时。' }
  }
  diagResult.value = { tool: tool.label, ...results[tool.id] }
  diagRunning.value = false
}

function resetFilters() {
  activeCategory.value = 'all'
  levelFilter.value = ''
}

async function loadData() {
  loading.value = true
  try {
    logs.value = await getLogs()
  } catch (e) {
    console.error('加载日志失败', e)
  } finally {
    loading.value = false
  }
}

onMounted(loadData)
</script>

<template>
  <div>
    <!-- 诊断工具区 -->
    <div class="card mb-16">
      <div class="card-title">诊断工具</div>
      <div class="diag-grid">
        <div v-for="tool in diagTools" :key="tool.id" class="diag-item">
          <div class="diag-info">
            <div class="diag-label">{{ tool.label }}</div>
            <div class="diag-desc">{{ tool.desc }}</div>
          </div>
          <button class="btn btn-sm btn-primary" :disabled="diagRunning" @click="runDiag(tool)">执行</button>
        </div>
      </div>
      <div v-if="diagResult" class="alert mt-16" :class="diagResult.ok ? 'alert-success' : 'alert-warning'">
        <strong>{{ diagResult.tool }}：</strong>{{ diagResult.message }}
      </div>
    </div>

    <div class="card">
      <!-- 统计 -->
      <div class="log-stats">
        <span>日志总数：<strong>{{ stats.total }}</strong></span>
        <span class="text-warning">警告：<strong>{{ stats.warning }}</strong></span>
        <span class="text-danger">错误：<strong>{{ stats.error }}</strong></span>
      </div>

      <!-- 分类标签 -->
      <div class="tabs">
        <div
          v-for="tab in categoryTabs"
          :key="tab.value"
          class="tab-item"
          :class="{ active: activeCategory === tab.value }"
          @click="activeCategory = tab.value"
        >{{ tab.label }}</div>
      </div>

      <!-- 筛选 -->
      <div class="toolbar">
        <div class="toolbar-item">
          <label>级别</label>
          <select class="form-select" v-model="levelFilter">
            <option v-for="o in levelOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
          </select>
        </div>
        <div class="spacer"></div>
        <button class="btn" @click="resetFilters">重置</button>
        <button class="btn" @click="loadData">刷新</button>
      </div>

      <!-- 日志列表 -->
      <div class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>时间</th>
              <th>分类</th>
              <th>级别</th>
              <th>关联节点</th>
              <th>日志内容</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="loading">
              <td colspan="5" class="text-center text-muted">加载中...</td>
            </tr>
            <tr v-else-if="!filteredLogs.length">
              <td colspan="5" class="text-center text-muted">暂无日志</td>
            </tr>
            <tr v-for="log in filteredLogs" :key="log.id">
              <td class="text-secondary" style="white-space: nowrap">{{ log.time }}</td>
              <td>{{ categoryLabel[log.category] || log.category }}</td>
              <td>
                <span class="status-badge" :class="levelMap[log.level]?.class || 'status-gray'">
                  {{ levelMap[log.level]?.label || log.level }}
                </span>
              </td>
              <td class="text-secondary">{{ log.node || '-' }}</td>
              <td>{{ log.message }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<style scoped>
.log-stats {
  display: flex;
  gap: 24px;
  font-size: 13px;
  color: var(--text-secondary);
  margin-bottom: 14px;
}

.log-stats strong {
  color: var(--text-primary);
  margin-left: 2px;
}

.diag-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.diag-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 14px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius);
  gap: 12px;
}

.diag-label {
  font-size: 14px;
  font-weight: 500;
  margin-bottom: 2px;
}

.diag-desc {
  font-size: 12px;
  color: var(--text-muted);
}
</style>
