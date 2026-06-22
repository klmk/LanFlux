<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute } from 'vue-router'
import { getPolicies, getPolicyByNode, updatePolicy } from '../api.js'
import StatusBadge from '../components/StatusBadge.vue'

const route = useRoute()

const loading = ref(true)
const nodes = ref([])
const segments = ref([])
const policies = ref([])

const selectedNodeId = ref(null)
const policyLoading = ref(false)

// 当前编辑的策略
const editMode = ref('deny') // deny / specified / all
const selectedSegmentIds = ref([])
const saveResult = ref(null)
const saving = ref(false)

const modeOptions = [
  { value: 'deny', label: '禁止访问', desc: '该节点不可访问任何网段' },
  { value: 'specified', label: '指定网段', desc: '仅可访问下方勾选的网段' },
  { value: 'all', label: '全部网段', desc: '可访问所有已启用的网段' }
]

const selectedNode = computed(() => nodes.value.find((n) => n.id === selectedNodeId.value))

const currentNodePolicy = computed(() => policies.value.find((p) => p.nodeId === selectedNodeId.value))

// 可选网段列表（已启用的网段，排除该节点自己上报的）
const availableSegments = computed(() => {
  return segments.value.filter((s) => s.status === 'enabled' && s.nodeId !== selectedNodeId.value)
})

function modeLabel(mode) {
  const o = modeOptions.find((m) => m.value === mode)
  return o ? o.label : mode
}

async function selectNode(nodeId) {
  selectedNodeId.value = nodeId
  saveResult.value = null
  policyLoading.value = true
  try {
    const detail = await getPolicyByNode(nodeId)
    editMode.value = detail.accessMode || 'deny'
    selectedSegmentIds.value = [...(detail.selectedSegments || [])]
    // 如果返回了 allSegments，更新本地 segments
    if (detail.allSegments) {
      segments.value = detail.allSegments
    }
  } catch (e) {
    console.error('加载策略失败', e)
  } finally {
    policyLoading.value = false
  }
}

function toggleSegment(segId) {
  const idx = selectedSegmentIds.value.indexOf(segId)
  if (idx >= 0) {
    selectedSegmentIds.value.splice(idx, 1)
  } else {
    selectedSegmentIds.value.push(segId)
  }
}

function selectAll() {
  selectedSegmentIds.value = availableSegments.value.map((s) => s.id)
}

function clearAll() {
  selectedSegmentIds.value = []
}

async function savePolicy() {
  if (!selectedNodeId.value) return
  saving.value = true
  saveResult.value = null
  try {
    const data = {
      accessMode: editMode.value,
      selectedSegments: editMode.value === 'specified' ? selectedSegmentIds.value : []
    }
    const res = await updatePolicy(selectedNodeId.value, data)
    saveResult.value = {
      success: true,
      pushed: res.pushed !== false,
      routeCount: res.routeCount || 0,
      message: res.pushed !== false
        ? `权限已保存并成功下发，共更新 ${res.routeCount || 0} 条路由`
        : '权限已保存，但路由下发失败，请检查节点连接状态'
    }
    // 更新本地策略缓存
    const p = policies.value.find((x) => x.nodeId === selectedNodeId.value)
    if (p) {
      p.accessMode = editMode.value
      p.selectedSegments = [...selectedSegmentIds.value]
    }
  } catch (e) {
    saveResult.value = { success: false, message: '保存失败：' + (e.message || e) }
  } finally {
    saving.value = false
  }
}

async function loadData() {
  loading.value = true
  try {
    const res = await getPolicies()
    nodes.value = res.nodes || []
    segments.value = res.segments || []
    policies.value = res.policies || []
    // 如果 URL 带有 node 参数，自动选中
    const queryNode = route.query.node
    if (queryNode && nodes.value.find((n) => n.id === queryNode)) {
      selectNode(queryNode)
    } else if (nodes.value.length) {
      selectNode(nodes.value[0].id)
    }
  } catch (e) {
    console.error('加载权限列表失败', e)
  } finally {
    loading.value = false
  }
}

onMounted(loadData)
</script>

<template>
  <div v-if="loading" class="loading-state">加载中...</div>
  <div v-else class="policy-layout">
    <!-- 左侧：访问者节点列表 -->
    <div class="card policy-left">
      <div class="card-title">访问者节点</div>
      <div class="node-list">
        <div
          v-for="node in nodes"
          :key="node.id"
          class="node-item"
          :class="{ active: node.id === selectedNodeId }"
          @click="selectNode(node.id)"
        >
          <div class="node-item-name">{{ node.name }}</div>
          <div class="node-item-info">
            <StatusBadge :status="node.status" type="node" />
            <span class="text-muted" style="margin-left: 6px">{{ modeLabel(currentNodePolicy && node.id === selectedNodeId ? editMode : (policies.find(p => p.nodeId === node.id) || {}).accessMode) }}</span>
          </div>
        </div>
        <div v-if="!nodes.length" class="empty-state">暂无访问者节点</div>
      </div>
    </div>

    <!-- 右侧：权限配置面板 -->
    <div class="card policy-right">
      <div class="card-title">
        <span>权限配置{{ selectedNode ? ' - ' + selectedNode.name : '' }}</span>
      </div>

      <div v-if="policyLoading" class="loading-state">加载策略中...</div>
      <template v-else-if="selectedNodeId">
        <!-- 访问模式 -->
        <div class="form-group">
          <label class="form-label">访问模式</label>
          <div class="mode-options">
            <label
              v-for="o in modeOptions"
              :key="o.value"
              class="mode-option"
              :class="{ active: editMode === o.value }"
            >
              <input type="radio" :value="o.value" v-model="editMode" />
              <span class="mode-option-label">{{ o.label }}</span>
              <span class="mode-option-desc">{{ o.desc }}</span>
            </label>
          </div>
        </div>

        <!-- 可访问网段列表（仅 specified 模式显示） -->
        <div v-if="editMode === 'specified'" class="form-group">
          <div class="flex-between mb-16">
            <label class="form-label" style="margin: 0">可访问网段列表（已选择 {{ selectedSegmentIds.length }} 个）</label>
            <div class="gap-8 flex">
              <button class="btn btn-sm" @click="selectAll">全选</button>
              <button class="btn btn-sm" @click="clearAll">清空</button>
            </div>
          </div>
          <div class="check-list">
            <label
              v-for="seg in availableSegments"
              :key="seg.id"
              class="check-item"
            >
              <input
                type="checkbox"
                :checked="selectedSegmentIds.includes(seg.id)"
                @change="toggleSegment(seg.id)"
              />
              <span>{{ seg.name }}</span>
              <span class="text-muted mono" style="margin-left: auto">{{ seg.mappedCidr }}</span>
            </label>
            <div v-if="!availableSegments.length" class="empty-state">暂无可选网段</div>
          </div>
        </div>

        <!-- 全部网段提示 -->
        <div v-if="editMode === 'all'" class="alert alert-info">
          该节点将可访问所有已启用的网段（共 {{ availableSegments.length }} 个）。
        </div>

        <!-- 禁止访问提示 -->
        <div v-if="editMode === 'deny'" class="alert alert-warning">
          该节点将被禁止访问任何网段，保存后相关路由将被撤销。
        </div>

        <!-- 保存按钮 -->
        <div class="flex gap-8 mt-16">
          <button class="btn btn-primary" :disabled="saving" @click="savePolicy">
            {{ saving ? '保存中...' : '保存并下发' }}
          </button>
        </div>

        <!-- 下发结果 -->
        <div v-if="saveResult" class="alert mt-16" :class="saveResult.success ? 'alert-success' : 'alert-error'">
          {{ saveResult.message }}
        </div>
      </template>
      <div v-else class="empty-state">请从左侧选择一个访问者节点</div>
    </div>
  </div>
</template>

<style scoped>
.policy-layout {
  display: flex;
  gap: 16px;
  align-items: flex-start;
}

.policy-left {
  width: 280px;
  flex-shrink: 0;
}

.policy-right {
  flex: 1;
  min-width: 0;
}

.node-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.node-item {
  padding: 10px 12px;
  border-radius: var(--radius);
  cursor: pointer;
  border: 1px solid transparent;
  transition: all 0.15s;
}

.node-item:hover {
  background: #f8fafc;
}

.node-item.active {
  background: var(--color-primary-light);
  border-color: var(--color-primary);
}

.node-item-name {
  font-size: 14px;
  font-weight: 500;
  margin-bottom: 4px;
}

.node-item-info {
  display: flex;
  align-items: center;
  font-size: 12px;
}

.mode-options {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.mode-option {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 10px 12px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius);
  cursor: pointer;
  transition: all 0.15s;
}

.mode-option:hover {
  border-color: var(--color-primary);
}

.mode-option.active {
  border-color: var(--color-primary);
  background: var(--color-primary-light);
}

.mode-option input[type='radio'] {
  margin-top: 2px;
  cursor: pointer;
}

.mode-option-label {
  font-size: 14px;
  font-weight: 500;
}

.mode-option-desc {
  display: block;
  font-size: 12px;
  color: var(--text-muted);
  margin-left: 20px;
  width: 100%;
}
</style>
