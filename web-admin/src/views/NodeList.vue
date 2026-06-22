<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { getNodes, disableNode, enableNode, kickNode } from '../api.js'
import StatusBadge from '../components/StatusBadge.vue'
import ConfirmDialog from '../components/ConfirmDialog.vue'

const router = useRouter()

const loading = ref(true)
const allNodes = ref([])

// 筛选条件
const filters = ref({
  role: '',
  status: '',
  os: '',
  keyword: ''
})

// 分页
const currentPage = ref(1)
const pageSize = ref(10)

// 确认弹窗
const confirmVisible = ref(false)
const confirmTitle = ref('')
const confirmContent = ref('')
const confirmType = ref('primary')
const pendingAction = ref(null)

const roleOptions = [
  { value: '', label: '全部角色' },
  { value: 'client', label: '客户端' },
  { value: 'agent', label: '实施端' }
]

const statusOptions = [
  { value: '', label: '全部状态' },
  { value: 'networked', label: '已组网' },
  { value: 'connected', label: '已连接' },
  { value: 'connecting', label: '连接中' },
  { value: 'partial', label: '部分异常' },
  { value: 'disconnected', label: '未连接' },
  { value: 'offline', label: '已断开' }
]

const osOptions = computed(() => {
  const set = new Set(allNodes.value.map((n) => n.os))
  return [{ value: '', label: '全部系统' }, ...Array.from(set).map((v) => ({ value: v, label: v }))]
})

// 过滤后的节点
const filteredNodes = computed(() => {
  return allNodes.value.filter((n) => {
    if (filters.value.role && n.role !== filters.value.role) return false
    if (filters.value.status && n.status !== filters.value.status) return false
    if (filters.value.os && n.os !== filters.value.os) return false
    if (filters.value.keyword) {
      const kw = filters.value.keyword.toLowerCase()
      const hit =
        n.name.toLowerCase().includes(kw) ||
        n.id.toLowerCase().includes(kw) ||
        (n.remark || '').toLowerCase().includes(kw) ||
        (n.virtualIp || '').toLowerCase().includes(kw)
      if (!hit) return false
    }
    return true
  })
})

const totalPages = computed(() => Math.max(1, Math.ceil(filteredNodes.value.length / pageSize.value)))

const pagedNodes = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value
  return filteredNodes.value.slice(start, start + pageSize.value)
})

function onFilterChange() {
  currentPage.value = 1
}

function resetFilters() {
  filters.value = { role: '', status: '', os: '', keyword: '' }
  currentPage.value = 1
}

function goDetail(id) {
  router.push(`/nodes/${id}`)
}

function roleLabel(role) {
  return role === 'agent' ? '实施端' : '客户端'
}

function isDisabled(node) {
  return node.status === 'offline' || node.status === 'disconnected'
}

// 操作：禁用 / 启用 / 踢下线
function handleDisable(node) {
  confirmTitle.value = '禁用节点'
  confirmContent.value = `确认要禁用节点「${node.name}」吗？\n禁用后该节点将无法接入组网。`
  confirmType.value = 'danger'
  pendingAction.value = async () => {
    await disableNode(node.id)
    node.status = 'offline'
  }
  confirmVisible.value = true
}

function handleEnable(node) {
  confirmTitle.value = '启用节点'
  confirmContent.value = `确认要启用节点「${node.name}」吗？`
  confirmType.value = 'primary'
  pendingAction.value = async () => {
    await enableNode(node.id)
    node.status = 'connecting'
  }
  confirmVisible.value = true
}

function handleKick(node) {
  confirmTitle.value = '踢下线'
  confirmContent.value = `确认要将节点「${node.name}」踢下线吗？\n该操作会立即断开当前连接。`
  confirmType.value = 'danger'
  pendingAction.value = async () => {
    await kickNode(node.id)
    node.status = 'disconnected'
  }
  confirmVisible.value = true
}

async function confirmAction() {
  confirmVisible.value = false
  if (pendingAction.value) {
    try {
      await pendingAction.value()
    } catch (e) {
      console.error('操作失败', e)
    }
    pendingAction.value = null
  }
}

function changePage(p) {
  if (p < 1 || p > totalPages.value) return
  currentPage.value = p
}

async function loadData() {
  loading.value = true
  try {
    allNodes.value = await getNodes()
  } catch (e) {
    console.error('加载节点列表失败', e)
  } finally {
    loading.value = false
  }
}

onMounted(loadData)
</script>

<template>
  <div class="card">
    <!-- 筛选栏 -->
    <div class="toolbar">
      <div class="toolbar-item">
        <label>角色</label>
        <select class="form-select" v-model="filters.role" @change="onFilterChange">
          <option v-for="o in roleOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>
      </div>
      <div class="toolbar-item">
        <label>状态</label>
        <select class="form-select" v-model="filters.status" @change="onFilterChange">
          <option v-for="o in statusOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>
      </div>
      <div class="toolbar-item">
        <label>操作系统</label>
        <select class="form-select" v-model="filters.os" @change="onFilterChange">
          <option v-for="o in osOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>
      </div>
      <div class="toolbar-item">
        <label>关键词</label>
        <input
          class="form-input"
          v-model="filters.keyword"
          @input="onFilterChange"
          placeholder="名称 / ID / IP / 备注"
        />
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
            <th>节点名称</th>
            <th>节点 ID</th>
            <th>角色</th>
            <th>状态</th>
            <th>操作系统</th>
            <th>虚拟 IP</th>
            <th>上报网段数</th>
            <th>最后在线时间</th>
            <th>备注</th>
            <th>操作</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="loading">
            <td colspan="10" class="text-center text-muted">加载中...</td>
          </tr>
          <tr v-else-if="!pagedNodes.length">
            <td colspan="10" class="text-center text-muted">暂无数据</td>
          </tr>
          <tr v-for="node in pagedNodes" :key="node.id">
            <td>
              <a @click="goDetail(node.id)">{{ node.name }}</a>
            </td>
            <td class="mono text-secondary">{{ node.id }}</td>
            <td>{{ roleLabel(node.role) }}</td>
            <td><StatusBadge :status="node.status" type="node" /></td>
            <td class="text-secondary">{{ node.os }}</td>
            <td class="mono">{{ node.virtualIp }}</td>
            <td class="text-center">{{ node.segmentCount }}</td>
            <td class="text-secondary">{{ node.lastOnline }}</td>
            <td class="text-secondary">{{ node.remark || '-' }}</td>
            <td class="actions-cell">
              <button class="btn-link" @click="goDetail(node.id)">详情</button>
              <button
                v-if="!isDisabled(node)"
                class="btn-link"
                @click="handleDisable(node)"
              >禁用</button>
              <button
                v-else
                class="btn-link"
                @click="handleEnable(node)"
              >启用</button>
              <button
                class="btn-link danger"
                :disabled="isDisabled(node)"
                @click="handleKick(node)"
              >踢下线</button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- 分页 -->
    <div class="pagination">
      <span class="page-info">共 {{ filteredNodes.length }} 条</span>
      <button class="page-btn" :disabled="currentPage === 1" @click="changePage(1)">首页</button>
      <button class="page-btn" :disabled="currentPage === 1" @click="changePage(currentPage - 1)">上一页</button>
      <span class="page-info">{{ currentPage }} / {{ totalPages }}</span>
      <button class="page-btn" :disabled="currentPage === totalPages" @click="changePage(currentPage + 1)">下一页</button>
      <button class="page-btn" :disabled="currentPage === totalPages" @click="changePage(totalPages)">末页</button>
    </div>

    <!-- 确认弹窗 -->
    <ConfirmDialog
      :visible="confirmVisible"
      :title="confirmTitle"
      :content="confirmContent"
      :confirm-type="confirmType"
      @confirm="confirmAction"
      @cancel="confirmVisible = false"
    />
  </div>
</template>
