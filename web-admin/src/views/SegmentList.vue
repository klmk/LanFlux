<script setup>
import { ref, computed, onMounted } from 'vue'
import { getSegments, enableSegment, disableSegment, remapSegment, updateSegment } from '../api.js'
import StatusBadge from '../components/StatusBadge.vue'
import ConfirmDialog from '../components/ConfirmDialog.vue'

const loading = ref(true)
const segments = ref([])

// 筛选
const filters = ref({ status: '', keyword: '' })

const statusOptions = [
  { value: '', label: '全部状态' },
  { value: 'enabled', label: '已启用' },
  { value: 'disabled', label: '已停用' },
  { value: 'pending', label: '待确认' },
  { value: 'error', label: '异常' }
]

const filteredSegments = computed(() => {
  return segments.value.filter((s) => {
    if (filters.value.status && s.status !== filters.value.status) return false
    if (filters.value.keyword) {
      const kw = filters.value.keyword.toLowerCase()
      const hit =
        s.name.toLowerCase().includes(kw) ||
        s.nodeName.toLowerCase().includes(kw) ||
        s.realCidr.toLowerCase().includes(kw) ||
        s.mappedCidr.toLowerCase().includes(kw)
      if (!hit) return false
    }
    return true
  })
})

// 确认弹窗
const confirmVisible = ref(false)
const confirmTitle = ref('')
const confirmContent = ref('')
const confirmType = ref('primary')
const pendingAction = ref(null)

// 修改名称弹窗
const renameVisible = ref(false)
const renameTarget = ref(null)
const renameValue = ref('')

// 访问权限弹窗
const accessVisible = ref(false)
const accessTarget = ref(null)

function handleEnable(seg) {
  confirmTitle.value = '确认启用网段'
  confirmContent.value = `确认要启用网段「${seg.name}」吗？\n启用后将向相关节点下发映射路由。`
  confirmType.value = 'primary'
  pendingAction.value = async () => {
    await enableSegment(seg.id)
    seg.status = 'enabled'
  }
  confirmVisible.value = true
}

function handleDisable(seg) {
  confirmTitle.value = '停用网段'
  confirmContent.value = `确认要停用网段「${seg.name}」吗？\n停用后相关节点将无法访问该网段。`
  confirmType.value = 'danger'
  pendingAction.value = async () => {
    await disableSegment(seg.id)
    seg.status = 'disabled'
  }
  confirmVisible.value = true
}

function handleRemap(seg) {
  confirmTitle.value = '重新分配映射网段'
  confirmContent.value =
    `确认要为网段「${seg.name}」重新分配映射网段吗？\n` +
    `当前映射网段：${seg.mappedCidr}\n` +
    `重新分配后，所有已下发的路由将更新，可能短暂影响正在进行的会话。`
  confirmType.value = 'danger'
  pendingAction.value = async () => {
    const res = await remapSegment(seg.id)
    if (res && res.newMappedCidr) {
      seg.mappedCidr = res.newMappedCidr
    }
  }
  confirmVisible.value = true
}

function openRename(seg) {
  renameTarget.value = seg
  renameValue.value = seg.name
  renameVisible.value = true
}

async function confirmRename() {
  if (!renameValue.value.trim()) return
  try {
    await updateSegment(renameTarget.value.id, { name: renameValue.value.trim() })
    renameTarget.value.name = renameValue.value.trim()
  } catch (e) {
    console.error('修改名称失败', e)
  }
  renameVisible.value = false
}

function openAccess(seg) {
  accessTarget.value = seg
  accessVisible.value = true
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

function resetFilters() {
  filters.value = { status: '', keyword: '' }
}

async function loadData() {
  loading.value = true
  try {
    segments.value = await getSegments()
  } catch (e) {
    console.error('加载网段列表失败', e)
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
        <label>状态</label>
        <select class="form-select" v-model="filters.status">
          <option v-for="o in statusOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>
      </div>
      <div class="toolbar-item">
        <label>关键词</label>
        <input class="form-input" v-model="filters.keyword" placeholder="名称 / 节点 / 网段地址" />
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
            <th>网段名称</th>
            <th>所属节点</th>
            <th>真实网段</th>
            <th>映射网段</th>
            <th>状态</th>
            <th>是否冲突</th>
            <th>备注</th>
            <th>操作</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="loading">
            <td colspan="8" class="text-center text-muted">加载中...</td>
          </tr>
          <tr v-else-if="!filteredSegments.length">
            <td colspan="8" class="text-center text-muted">暂无数据</td>
          </tr>
          <tr v-for="seg in filteredSegments" :key="seg.id">
            <td>{{ seg.name }}</td>
            <td>{{ seg.nodeName }}</td>
            <td class="mono">{{ seg.realCidr }}</td>
            <td class="mono">{{ seg.mappedCidr }}</td>
            <td><StatusBadge :status="seg.status" type="segment" /></td>
            <td>
              <span v-if="seg.conflict" class="status-badge status-red">冲突</span>
              <span v-else class="text-secondary">无</span>
            </td>
            <td class="text-secondary">{{ seg.remark || '-' }}</td>
            <td class="actions-cell">
              <button
                v-if="seg.status !== 'enabled'"
                class="btn-link"
                @click="handleEnable(seg)"
              >确认启用</button>
              <button
                v-if="seg.status === 'enabled'"
                class="btn-link danger"
                @click="handleDisable(seg)"
              >停用</button>
              <button class="btn-link" @click="openRename(seg)">修改名称</button>
              <button class="btn-link danger" @click="handleRemap(seg)">重新分配映射</button>
              <button class="btn-link" @click="openAccess(seg)">查看访问权限</button>
            </td>
          </tr>
        </tbody>
      </table>
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

    <!-- 修改名称弹窗 -->
    <teleport to="body">
      <div v-if="renameVisible" class="confirm-overlay" @click.self="renameVisible = false">
        <div class="confirm-dialog">
          <div class="confirm-header">
            <span class="confirm-title">修改网段名称</span>
            <span class="confirm-close" @click="renameVisible = false">×</span>
          </div>
          <div class="confirm-body">
            <div class="form-group">
              <label class="form-label">网段名称</label>
              <input class="form-input" v-model="renameValue" placeholder="请输入网段名称" />
            </div>
          </div>
          <div class="confirm-footer">
            <button class="btn" @click="renameVisible = false">取消</button>
            <button class="btn btn-primary" @click="confirmRename">保存</button>
          </div>
        </div>
      </div>
    </teleport>

    <!-- 查看访问权限弹窗 -->
    <teleport to="body">
      <div v-if="accessVisible" class="confirm-overlay" @click.self="accessVisible = false">
        <div class="confirm-dialog" style="width: 480px">
          <div class="confirm-header">
            <span class="confirm-title">访问权限 - {{ accessTarget?.name }}</span>
            <span class="confirm-close" @click="accessVisible = false">×</span>
          </div>
          <div class="confirm-body">
            <div class="desc-list">
              <div class="desc-label">真实网段</div>
              <div class="desc-value mono">{{ accessTarget?.realCidr }}</div>
              <div class="desc-label">映射网段</div>
              <div class="desc-value mono">{{ accessTarget?.mappedCidr }}</div>
              <div class="desc-label">所属节点</div>
              <div class="desc-value">{{ accessTarget?.nodeName }}</div>
              <div class="desc-label">状态</div>
              <div class="desc-value"><StatusBadge :status="accessTarget?.status" type="segment" /></div>
            </div>
            <div class="alert alert-info mt-16">
              该网段的访问权限由各访问者节点的权限策略决定，可在「权限配置」页面为每个节点单独设置。
            </div>
          </div>
          <div class="confirm-footer">
            <button class="btn" @click="accessVisible = false">关闭</button>
            <button class="btn btn-primary" @click="accessVisible = false; $router.push('/policies')">前往权限配置</button>
          </div>
        </div>
      </div>
    </teleport>
  </div>
</template>

<style scoped>
.confirm-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.confirm-dialog {
  background: #fff;
  border-radius: var(--radius);
  box-shadow: var(--shadow-md);
  width: 420px;
  max-width: 90vw;
  overflow: hidden;
}

.confirm-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 18px;
  border-bottom: 1px solid var(--border-color);
}

.confirm-title {
  font-size: 15px;
  font-weight: 600;
}

.confirm-close {
  font-size: 22px;
  line-height: 1;
  color: var(--text-muted);
  cursor: pointer;
  padding: 0 4px;
}

.confirm-close:hover {
  color: var(--text-primary);
}

.confirm-body {
  padding: 20px 18px;
  font-size: 14px;
  line-height: 1.6;
}

.confirm-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 12px 18px 16px;
}
</style>
