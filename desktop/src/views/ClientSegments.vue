<script setup>
import { ref, onMounted } from 'vue'
import StatusBadge from '../components/StatusBadge.vue'
import { useToast } from '../composables/useToast.js'
import { scanInterfaces, reportSegment, refreshReportedSegments } from '../tauri.js'

const toast = useToast()

const loading = ref(false)
const scanning = ref(false)
const reporting = ref(false)
const interfaces = ref([])
const selectedIds = ref(new Set())
const segmentNames = ref({})
const segmentRemarks = ref({})
const reportedSegments = ref([])

async function scan() {
  scanning.value = true
  try {
    interfaces.value = await scanInterfaces()
    // 默认勾选推荐的网卡
    selectedIds.value = new Set(
      interfaces.value
        .filter((i) => i.recommended)
        .map((_, idx) => idx)
    )
  } catch (e) {
    console.error('扫描网卡失败', e)
    toast.error('扫描网卡失败: ' + (e.message || e))
  } finally {
    scanning.value = false
  }
}

function toggleSelect(idx) {
  if (selectedIds.value.has(idx)) {
    selectedIds.value.delete(idx)
  } else {
    selectedIds.value.add(idx)
  }
  // 触发响应式更新
  selectedIds.value = new Set(selectedIds.value)
}

async function reportSelected() {
  if (selectedIds.value.size === 0) {
    toast.warning('请至少勾选一个网卡')
    return
  }

  reporting.value = true
  let successCount = 0
  let failCount = 0
  try {
    for (const idx of selectedIds.value) {
      const iface = interfaces.value[idx]
      const name = segmentNames.value[idx] || `${iface.name} 网段`
      const remark = segmentRemarks.value[idx] || ''

      try {
        const res = await reportSegment(name, iface.cidr, remark || null)
        successCount++
        // 展示上报返回的映射网段
        toast.success(`${name} 上报成功，映射网段: ${res.mapped_cidr || '-'}`)
      } catch (e) {
        failCount++
        console.error('上报失败', e)
        toast.error(`${name} 上报失败: ` + (e.message || e))
      }
    }

    if (failCount === 0 && successCount > 0) {
      toast.success(`全部上报完成，共 ${successCount} 个网段`)
    } else if (failCount > 0) {
      toast.warning(`上报完成：成功 ${successCount} 个，失败 ${failCount} 个`)
    }

    await loadReported()
  } finally {
    reporting.value = false
  }
}

async function loadReported() {
  loading.value = true
  try {
    reportedSegments.value = await refreshReportedSegments()
  } catch (e) {
    console.error('加载已上报网段失败', e)
    reportedSegments.value = []
    toast.error('加载已上报网段失败: ' + (e.message || e))
  } finally {
    loading.value = false
  }
}

async function init() {
  await scan()
  await loadReported()
}

onMounted(init)
</script>

<template>
  <div>
    <!-- 网卡扫描结果 -->
    <div class="card">
      <div class="card-title">
        <span>本机网卡扫描</span>
        <button class="btn btn-sm" @click="scan" :disabled="scanning">
          {{ scanning ? '扫描中...' : '重新扫描' }}
        </button>
      </div>

      <div v-if="scanning" class="loading-state">正在扫描本机网卡...</div>
      <div v-else-if="!interfaces.length" class="empty-state">
        <div class="empty-icon">◉</div>
        <div>未扫描到可用网卡</div>
      </div>
      <div v-else class="iface-list">
        <div
          v-for="(iface, idx) in interfaces"
          :key="idx"
          class="iface-item"
          :class="{ selected: selectedIds.has(idx) }"
        >
          <div class="iface-check">
            <input
              type="checkbox"
              :checked="selectedIds.has(idx)"
              @change="toggleSelect(idx)"
            />
          </div>
          <div class="iface-info">
            <div class="iface-header">
              <span class="iface-name">{{ iface.name }}</span>
              <span class="iface-type">{{ iface.iface_type }}</span>
              <span v-if="iface.recommended" class="status-badge status-green">推荐</span>
            </div>
            <div class="iface-details">
              <span class="detail-item">
                <span class="detail-label">IP:</span>
                <span class="mono">{{ iface.ip_address || '-' }}</span>
              </span>
              <span class="detail-item">
                <span class="detail-label">网段:</span>
                <span class="mono">{{ iface.cidr || '-' }}</span>
              </span>
              <span class="detail-item">
                <span class="detail-label">网关:</span>
                <span class="mono">{{ iface.gateway || '-' }}</span>
              </span>
            </div>
          </div>
          <div v-if="selectedIds.has(idx)" class="iface-form">
            <input
              class="form-input"
              v-model="segmentNames[idx]"
              placeholder="网段名称（可选）"
            />
            <input
              class="form-input"
              v-model="segmentRemarks[idx]"
              placeholder="备注（可选）"
            />
          </div>
        </div>
      </div>
    </div>

    <!-- 上报按钮 -->
    <div v-if="interfaces.length" class="card" style="text-align: center">
      <button
        class="btn btn-primary"
        @click="reportSelected"
        :disabled="reporting || selectedIds.size === 0"
      >
        {{ reporting ? '上报中...' : `上报选中的网段 (${selectedIds.size})` }}
      </button>
    </div>

    <!-- 已上报网段列表 -->
    <div class="card">
      <div class="card-title">
        <span>已上报网段 ({{ reportedSegments.length }})</span>
        <button class="btn btn-sm" @click="loadReported" :disabled="loading">刷新</button>
      </div>
      <div v-if="loading" class="loading-state">加载中...</div>
      <div v-else-if="!reportedSegments.length" class="empty-state">
        <div class="empty-icon">▦</div>
        <div>暂无已上报网段</div>
      </div>
      <div v-else class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>网段名称</th>
              <th>真实网段</th>
              <th>映射网段</th>
              <th>状态</th>
              <th>备注</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="seg in reportedSegments" :key="seg.id">
              <td>{{ seg.name }}</td>
              <td class="mono">{{ seg.real_cidr }}</td>
              <td class="mono">{{ seg.mapped_cidr || '-' }}</td>
              <td><StatusBadge :status="seg.status" type="segment" /></td>
              <td class="text-secondary">{{ seg.remark || '-' }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<style scoped>
.iface-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.iface-item {
  border: 1px solid var(--border-color);
  border-radius: var(--radius);
  padding: 12px 14px;
  transition: all 0.15s;
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  align-items: flex-start;
}

.iface-item.selected {
  border-color: var(--color-primary);
  background: var(--color-primary-light);
}

.iface-check {
  padding-top: 2px;
}

.iface-check input {
  cursor: pointer;
  width: 16px;
  height: 16px;
}

.iface-info {
  flex: 1;
  min-width: 200px;
}

.iface-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.iface-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.iface-type {
  font-size: 12px;
  color: var(--text-secondary);
  background: var(--status-gray-bg);
  padding: 1px 8px;
  border-radius: 10px;
}

.iface-details {
  display: flex;
  gap: 20px;
  flex-wrap: wrap;
}

.detail-item {
  font-size: 13px;
}

.detail-label {
  color: var(--text-muted);
  margin-right: 4px;
}

.iface-form {
  display: flex;
  gap: 8px;
  width: 100%;
  margin-top: 4px;
}

.iface-form .form-input {
  flex: 1;
}
</style>
