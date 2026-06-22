<script setup>
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { getDashboard } from '../api.js'
import StatusBadge from '../components/StatusBadge.vue'

const router = useRouter()

const loading = ref(true)
const data = ref({
  stats: { onlineNodes: 0, registeredSegments: 0, agentOnline: 0, alerts: 0 },
  recentNodes: [],
  recentAlerts: [],
  mappingOverview: []
})

const levelMap = {
  info: { label: '信息', class: 'status-blue' },
  warning: { label: '警告', class: 'status-orange' },
  error: { label: '错误', class: 'status-red' }
}

async function loadData() {
  loading.value = true
  try {
    data.value = await getDashboard()
  } catch (e) {
    console.error('加载仪表盘失败', e)
  } finally {
    loading.value = false
  }
}

function goNode(id) {
  router.push(`/nodes/${id}`)
}

function goNodes() {
  router.push('/nodes')
}

function goSegments() {
  router.push('/segments')
}

onMounted(loadData)
</script>

<template>
  <div>
    <div v-if="loading" class="loading-state">加载中...</div>
    <template v-else>
      <!-- 顶部状态卡片 -->
      <div class="stat-grid">
        <div class="stat-card">
          <div class="stat-icon blue">◉</div>
          <div>
            <div class="stat-value">{{ data.stats.onlineNodes }}</div>
            <div class="stat-label">在线节点</div>
          </div>
        </div>
        <div class="stat-card">
          <div class="stat-icon green">▦</div>
          <div>
            <div class="stat-value">{{ data.stats.registeredSegments }}</div>
            <div class="stat-label">已登记网段</div>
          </div>
        </div>
        <div class="stat-card">
          <div class="stat-icon cyan">◈</div>
          <div>
            <div class="stat-value">{{ data.stats.agentOnline }}</div>
            <div class="stat-label">实施端在线</div>
          </div>
        </div>
        <div class="stat-card">
          <div class="stat-icon red">!</div>
          <div>
            <div class="stat-value">{{ data.stats.alerts }}</div>
            <div class="stat-label">异常告警</div>
          </div>
        </div>
      </div>

      <!-- 中部：最近上线节点 + 最近异常事件 -->
      <div class="grid-2">
        <div class="card">
          <div class="card-title">
            <span>最近上线节点</span>
            <button class="btn btn-sm" @click="goNodes">查看全部</button>
          </div>
          <div class="table-wrap">
            <table class="data-table">
              <thead>
                <tr>
                  <th>节点名称</th>
                  <th>角色</th>
                  <th>状态</th>
                  <th>虚拟 IP</th>
                  <th>最后在线</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="node in data.recentNodes"
                  :key="node.id"
                  style="cursor: pointer"
                  @click="goNode(node.id)"
                >
                  <td>{{ node.name }}</td>
                  <td>{{ node.role === 'agent' ? '实施端' : '客户端' }}</td>
                  <td><StatusBadge :status="node.status" type="node" /></td>
                  <td class="mono">{{ node.virtualIp }}</td>
                  <td class="text-secondary">{{ node.lastOnline }}</td>
                </tr>
                <tr v-if="!data.recentNodes.length">
                  <td colspan="5" class="text-center text-muted">暂无数据</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>

        <div class="card">
          <div class="card-title">
            <span>最近异常事件</span>
            <button class="btn btn-sm" @click="$router.push('/logs')">查看日志</button>
          </div>
          <div class="alert-list">
            <div v-for="alert in data.recentAlerts" :key="alert.id" class="alert-row">
              <span class="status-badge" :class="levelMap[alert.level]?.class || 'status-gray'">
                {{ levelMap[alert.level]?.label || alert.level }}
              </span>
              <span class="alert-time">{{ alert.time }}</span>
              <span class="alert-msg">{{ alert.message }}</span>
            </div>
            <div v-if="!data.recentAlerts.length" class="empty-state">暂无异常事件</div>
          </div>
        </div>
      </div>

      <!-- 底部：网段映射概览 -->
      <div class="card">
        <div class="card-title">
          <span>网段映射概览</span>
          <button class="btn btn-sm" @click="goSegments">网段管理</button>
        </div>
        <div class="table-wrap">
          <table class="data-table">
            <thead>
              <tr>
                <th>网段名称</th>
                <th>所属节点</th>
                <th>真实网段</th>
                <th>映射网段</th>
                <th>状态</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="seg in data.mappingOverview" :key="seg.id">
                <td>{{ seg.name }}</td>
                <td>{{ seg.nodeName }}</td>
                <td class="mono">{{ seg.realCidr }}</td>
                <td class="mono">{{ seg.mappedCidr }}</td>
                <td><StatusBadge :status="seg.status" type="segment" /></td>
              </tr>
              <tr v-if="!data.mappingOverview.length">
                <td colspan="5" class="text-center text-muted">暂无数据</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.alert-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.alert-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 4px;
  border-bottom: 1px solid var(--border-color-light);
  font-size: 13px;
}

.alert-row:last-child {
  border-bottom: none;
}

.alert-time {
  color: var(--text-muted);
  font-size: 12px;
  white-space: nowrap;
  flex-shrink: 0;
}

.alert-msg {
  color: var(--text-primary);
  flex: 1;
}
</style>
