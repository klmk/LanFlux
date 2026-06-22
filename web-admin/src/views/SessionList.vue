<script setup>
import { ref, computed, onMounted } from 'vue'
import { getSessions } from '../api.js'
import StatusBadge from '../components/StatusBadge.vue'

const loading = ref(true)
const sessions = ref([])

// 筛选
const filters = ref({ protocol: '', status: '', keyword: '' })

const protocolOptions = [
  { value: '', label: '全部协议' },
  { value: 'TCP', label: 'TCP' },
  { value: 'UDP', label: 'UDP' }
]

const statusOptions = [
  { value: '', label: '全部状态' },
  { value: 'active', label: '活跃' },
  { value: 'idle', label: '空闲' },
  { value: 'closed', label: '已关闭' }
]

const statusMap = {
  active: { label: '活跃', class: 'status-green' },
  idle: { label: '空闲', class: 'status-yellow' },
  closed: { label: '已关闭', class: 'status-gray' }
}

const filteredSessions = computed(() => {
  return sessions.value.filter((s) => {
    if (filters.value.protocol && s.protocol !== filters.value.protocol) return false
    if (filters.value.status && s.status !== filters.value.status) return false
    if (filters.value.keyword) {
      const kw = filters.value.keyword.toLowerCase()
      const hit =
        s.sourceNode.toLowerCase().includes(kw) ||
        s.targetSegment.toLowerCase().includes(kw) ||
        s.targetAddress.toLowerCase().includes(kw) ||
        (s.customer || '').toLowerCase().includes(kw)
      if (!hit) return false
    }
    return true
  })
})

const stats = computed(() => ({
  total: sessions.value.length,
  active: sessions.value.filter((s) => s.status === 'active').length,
  tcp: sessions.value.filter((s) => s.protocol === 'TCP').length,
  udp: sessions.value.filter((s) => s.protocol === 'UDP').length
}))

function resetFilters() {
  filters.value = { protocol: '', status: '', keyword: '' }
}

async function loadData() {
  loading.value = true
  try {
    sessions.value = await getSessions()
  } catch (e) {
    console.error('加载会话列表失败', e)
  } finally {
    loading.value = false
  }
}

onMounted(loadData)
</script>

<template>
  <div>
    <!-- 统计概览 -->
    <div class="stat-grid">
      <div class="stat-card">
        <div class="stat-icon blue">⇆</div>
        <div>
          <div class="stat-value">{{ stats.total }}</div>
          <div class="stat-label">总会话数</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-icon green">●</div>
        <div>
          <div class="stat-value">{{ stats.active }}</div>
          <div class="stat-label">活跃会话</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-icon cyan">T</div>
        <div>
          <div class="stat-value">{{ stats.tcp }}</div>
          <div class="stat-label">TCP 会话</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-icon blue">U</div>
        <div>
          <div class="stat-value">{{ stats.udp }}</div>
          <div class="stat-label">UDP 会话</div>
        </div>
      </div>
    </div>

    <div class="card">
      <!-- 筛选栏 -->
      <div class="toolbar">
        <div class="toolbar-item">
          <label>协议</label>
          <select class="form-select" v-model="filters.protocol">
            <option v-for="o in protocolOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
          </select>
        </div>
        <div class="toolbar-item">
          <label>状态</label>
          <select class="form-select" v-model="filters.status">
            <option v-for="o in statusOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
          </select>
        </div>
        <div class="toolbar-item">
          <label>关键词</label>
          <input class="form-input" v-model="filters.keyword" placeholder="来源节点 / 目标网段 / 地址 / 客户" />
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
              <th>来源节点</th>
              <th>目标网段</th>
              <th>协议</th>
              <th>目标地址</th>
              <th>所属客户</th>
              <th>开始时间</th>
              <th>最近活动</th>
              <th>超时时间</th>
              <th>状态</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="loading">
              <td colspan="9" class="text-center text-muted">加载中...</td>
            </tr>
            <tr v-else-if="!filteredSessions.length">
              <td colspan="9" class="text-center text-muted">暂无数据</td>
            </tr>
            <tr v-for="s in filteredSessions" :key="s.id">
              <td>{{ s.sourceNode }}</td>
              <td>{{ s.targetSegment }}</td>
              <td>
                <span class="status-badge" :class="s.protocol === 'TCP' ? 'status-blue' : 'status-orange'">{{ s.protocol }}</span>
              </td>
              <td class="mono">{{ s.targetAddress }}</td>
              <td class="text-secondary">{{ s.customer || '-' }}</td>
              <td class="text-secondary">{{ s.startTime }}</td>
              <td class="text-secondary">{{ s.lastActivity }}</td>
              <td>
                <span v-if="s.protocol === 'UDP' && s.timeout" class="text-warning">{{ s.timeout }}</span>
                <span v-else class="text-muted">-</span>
              </td>
              <td>
                <span class="status-badge" :class="statusMap[s.status]?.class || 'status-gray'">
                  {{ statusMap[s.status]?.label || s.status }}
                </span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
