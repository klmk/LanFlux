<script setup>
import { ref, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { getNode } from '../api.js'
import StatusBadge from '../components/StatusBadge.vue'

const route = useRoute()
const router = useRouter()

const loading = ref(true)
const node = ref(null)

const routeStatusMap = {
  synced: { label: '已同步', class: 'status-green' },
  syncing: { label: '同步中', class: 'status-yellow' },
  failed: { label: '下发失败', class: 'status-red' },
  pending: { label: '待下发', class: 'status-gray' }
}

function formatBytes(bytes) {
  if (!bytes && bytes !== 0) return '-'
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  if (bytes < 1024 * 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + ' MB'
  return (bytes / 1024 / 1024 / 1024).toFixed(2) + ' GB'
}

function roleLabel(role) {
  return role === 'agent' ? '实施端' : '客户端'
}

function goPolicy() {
  router.push({ path: '/policies', query: { node: node.value.id } })
}

function goBack() {
  router.push('/nodes')
}

async function loadData() {
  loading.value = true
  try {
    node.value = await getNode(route.params.id)
  } catch (e) {
    console.error('加载节点详情失败', e)
  } finally {
    loading.value = false
  }
}

onMounted(loadData)
</script>

<template>
  <div>
    <div class="mb-16">
      <button class="btn" @click="goBack">← 返回节点列表</button>
    </div>

    <div v-if="loading" class="loading-state">加载中...</div>
    <template v-else-if="node">
      <!-- 基础信息 + 连接信息 -->
      <div class="grid-2 mb-16">
        <div class="card">
          <div class="card-title">基础信息</div>
          <div class="desc-list">
            <div class="desc-label">节点名称</div>
            <div class="desc-value">{{ node.name }}</div>
            <div class="desc-label">节点 ID</div>
            <div class="desc-value mono">{{ node.id }}</div>
            <div class="desc-label">角色</div>
            <div class="desc-value">{{ roleLabel(node.role) }}</div>
            <div class="desc-label">状态</div>
            <div class="desc-value"><StatusBadge :status="node.status" type="node" /></div>
            <div class="desc-label">操作系统</div>
            <div class="desc-value">{{ node.os }}</div>
            <div class="desc-label">虚拟 IP</div>
            <div class="desc-value mono">{{ node.virtualIp }}</div>
            <div class="desc-label">最后在线</div>
            <div class="desc-value">{{ node.lastOnline }}</div>
            <div class="desc-label">备注</div>
            <div class="desc-value">{{ node.remark || '-' }}</div>
          </div>
        </div>

        <div class="card">
          <div class="card-title">连接信息</div>
          <div v-if="node.connection" class="desc-list">
            <div class="desc-label">连接时间</div>
            <div class="desc-value">{{ node.connection.connectTime }}</div>
            <div class="desc-label">远端地址</div>
            <div class="desc-value mono">{{ node.connection.remoteAddr }}</div>
            <div class="desc-label">通信协议</div>
            <div class="desc-value">{{ node.connection.protocol }}</div>
            <div class="desc-label">下行流量</div>
            <div class="desc-value">{{ formatBytes(node.connection.rxBytes) }}</div>
            <div class="desc-label">上行流量</div>
            <div class="desc-value">{{ formatBytes(node.connection.txBytes) }}</div>
            <div class="desc-label">网络延迟</div>
            <div class="desc-value">{{ node.connection.latency }} ms</div>
          </div>
          <div v-else class="empty-state">节点未连接，无连接信息</div>
        </div>
      </div>

      <!-- 路由下发状态 -->
      <div class="card mb-16">
        <div class="card-title">路由下发状态</div>
        <div class="route-status-bar">
          <span class="status-badge" :class="routeStatusMap[node.routeStatus]?.class || 'status-gray'">
            {{ routeStatusMap[node.routeStatus]?.label || node.routeStatus }}
          </span>
          <span v-if="node.routeDetail" class="text-secondary mt-8" style="margin-left: 12px">
            共 {{ node.routeDetail.total }} 条路由，已同步 {{ node.routeDetail.synced }} 条，失败 {{ node.routeDetail.failed }} 条
          </span>
        </div>
      </div>

      <!-- 上报网段 + 可访问网段 -->
      <div class="grid-2 mb-16">
        <div class="card">
          <div class="card-title">上报网段列表</div>
          <div class="table-wrap">
            <table class="data-table">
              <thead>
                <tr>
                  <th>网段名称</th>
                  <th>真实网段</th>
                  <th>映射网段</th>
                  <th>状态</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="seg in node.reportedSegments" :key="seg.id">
                  <td>{{ seg.name }}</td>
                  <td class="mono">{{ seg.realCidr }}</td>
                  <td class="mono">{{ seg.mappedCidr }}</td>
                  <td><StatusBadge :status="seg.status" type="segment" /></td>
                </tr>
                <tr v-if="!node.reportedSegments || !node.reportedSegments.length">
                  <td colspan="4" class="text-center text-muted">暂无上报网段</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>

        <div class="card">
          <div class="card-title">
            <span>可访问网段列表</span>
            <button class="btn btn-sm btn-primary" @click="goPolicy">权限设置</button>
          </div>
          <div class="table-wrap">
            <table class="data-table">
              <thead>
                <tr>
                  <th>网段名称</th>
                  <th>所属节点</th>
                  <th>映射网段</th>
                  <th>状态</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="seg in node.accessibleSegments" :key="seg.id">
                  <td>{{ seg.name }}</td>
                  <td>{{ seg.nodeName }}</td>
                  <td class="mono">{{ seg.mappedCidr }}</td>
                  <td><StatusBadge :status="seg.status" type="segment" /></td>
                </tr>
                <tr v-if="!node.accessibleSegments || !node.accessibleSegments.length">
                  <td colspan="4" class="text-center text-muted">暂无可访问网段</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>

      <!-- 最近日志 -->
      <div class="card">
        <div class="card-title">
          <span>最近日志</span>
          <button class="btn btn-sm" @click="$router.push('/logs')">查看全部</button>
        </div>
        <div class="table-wrap">
          <table class="data-table">
            <thead>
              <tr>
                <th>时间</th>
                <th>级别</th>
                <th>内容</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="log in node.recentLogs" :key="log.id">
                <td class="text-secondary" style="white-space: nowrap">{{ log.time }}</td>
                <td>
                  <span
                    class="status-badge"
                    :class="log.level === 'error' ? 'status-red' : log.level === 'warning' ? 'status-orange' : 'status-blue'"
                  >{{ log.level === 'error' ? '错误' : log.level === 'warning' ? '警告' : '信息' }}</span>
                </td>
                <td>{{ log.message }}</td>
              </tr>
              <tr v-if="!node.recentLogs || !node.recentLogs.length">
                <td colspan="3" class="text-center text-muted">暂无日志</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.route-status-bar {
  display: flex;
  align-items: center;
}
</style>
