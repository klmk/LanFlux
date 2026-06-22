<script setup>
import { ref, computed, onMounted } from 'vue'
import { getPools, updatePools } from '../api.js'
import ConfirmDialog from '../components/ConfirmDialog.vue'

const loading = ref(true)
const saving = ref(false)
const form = ref({
  clientPool: '',
  segmentSize: 24,
  agentPool: '',
  serverVirtualIp: ''
})

const original = ref({})
const saveResult = ref(null)

// 是否有地址池已被使用（用于提示）
const poolUsed = computed(() => (original.value.clientPoolUsed || 0) > 0 || (original.value.agentPoolUsed || 0) > 0)

const hasChange = computed(() => {
  return (
    form.value.clientPool !== original.value.clientPool ||
    Number(form.value.segmentSize) !== Number(original.value.segmentSize) ||
    form.value.agentPool !== original.value.agentPool ||
    form.value.serverVirtualIp !== original.value.serverVirtualIp
  )
})

// 确认弹窗
const confirmVisible = ref(false)

const segmentSizeOptions = [16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28]

function handleSave() {
  if (poolUsed.value) {
    confirmVisible.value = true
    return
  }
  doSave()
}

async function doSave() {
  confirmVisible.value = false
  saving.value = true
  saveResult.value = null
  try {
    await updatePools({
      clientPool: form.value.clientPool,
      segmentSize: Number(form.value.segmentSize),
      agentPool: form.value.agentPool,
      serverVirtualIp: form.value.serverVirtualIp
    })
    original.value = { ...original.value, ...form.value, segmentSize: Number(form.value.segmentSize) }
    saveResult.value = { success: true, message: '地址池配置已保存' }
  } catch (e) {
    saveResult.value = { success: false, message: '保存失败：' + (e.message || e) }
  } finally {
    saving.value = false
  }
}

function handleReset() {
  form.value = {
    clientPool: original.value.clientPool || '',
    segmentSize: original.value.segmentSize || 24,
    agentPool: original.value.agentPool || '',
    serverVirtualIp: original.value.serverVirtualIp || ''
  }
  saveResult.value = null
}

async function loadData() {
  loading.value = true
  try {
    const data = await getPools()
    original.value = { ...data }
    form.value = {
      clientPool: data.clientPool || '',
      segmentSize: data.segmentSize || 24,
      agentPool: data.agentPool || '',
      serverVirtualIp: data.serverVirtualIp || ''
    }
  } catch (e) {
    console.error('加载地址池配置失败', e)
  } finally {
    loading.value = false
  }
}

onMounted(loadData)
</script>

<template>
  <div>
    <div v-if="loading" class="loading-state">加载中...</div>
    <template v-else>
      <!-- 使用情况概览 -->
      <div class="card mb-16">
        <div class="card-title">地址池使用情况</div>
        <div class="grid-2">
          <div class="pool-usage">
            <div class="pool-usage-label">客户映射地址池</div>
            <div class="pool-usage-value mono">{{ original.clientPool || '-' }}</div>
            <div class="pool-usage-bar">
              <div
                class="pool-usage-fill"
                :style="{ width: ((original.clientPoolUsed || 0) / (original.clientPoolTotal || 1) * 100) + '%' }"
              ></div>
            </div>
            <div class="text-secondary" style="font-size: 12px; margin-top: 4px">
              已使用 {{ original.clientPoolUsed || 0 }} / {{ original.clientPoolTotal || 0 }}
            </div>
          </div>
          <div class="pool-usage">
            <div class="pool-usage-label">实施端地址池</div>
            <div class="pool-usage-value mono">{{ original.agentPool || '-' }}</div>
            <div class="pool-usage-bar">
              <div
                class="pool-usage-fill"
                :style="{ width: ((original.agentPoolUsed || 0) / (original.agentPoolTotal || 1) * 100) + '%' }"
              ></div>
            </div>
            <div class="text-secondary" style="font-size: 12px; margin-top: 4px">
              已使用 {{ original.agentPoolUsed || 0 }} / {{ original.agentPoolTotal || 0 }}
            </div>
          </div>
        </div>
      </div>

      <!-- 配置表单 -->
      <div class="card">
        <div class="card-title">地址池配置</div>

        <div v-if="poolUsed" class="alert alert-warning">
          注意：地址池已被使用（客户池 {{ original.clientPoolUsed || 0 }} 个、实施端池 {{ original.agentPoolUsed || 0 }} 个）。修改地址池范围可能导致已分配的地址失效，不建议在运行中修改。
        </div>

        <div style="max-width: 560px">
          <div class="form-group">
            <label class="form-label">客户映射地址池<span class="required">*</span></label>
            <input class="form-input" v-model="form.clientPool" placeholder="例如 10.10.0.0/16" />
            <div class="form-hint">客户端节点虚拟 IP 及映射网段的分配地址池，使用 CIDR 格式</div>
          </div>

          <div class="form-group">
            <label class="form-label">单个映射网段大小<span class="required">*</span></label>
            <select class="form-select" v-model="form.segmentSize">
              <option v-for="s in segmentSizeOptions" :key="s" :value="s">/{{ s }}（{{ Math.pow(2, 32 - s) }} 个地址）</option>
            </select>
            <div class="form-hint">每个网段映射时分配的子网大小，数值越小单个网段地址越多</div>
          </div>

          <div class="form-group">
            <label class="form-label">实施端地址池<span class="required">*</span></label>
            <input class="form-input" v-model="form.agentPool" placeholder="例如 10.20.0.0/16" />
            <div class="form-hint">实施端节点虚拟 IP 的分配地址池，使用 CIDR 格式</div>
          </div>

          <div class="form-group">
            <label class="form-label">服务端虚拟地址<span class="required">*</span></label>
            <input class="form-input" v-model="form.serverVirtualIp" placeholder="例如 10.10.0.1" />
            <div class="form-hint">服务端在组网中的虚拟 IP 地址，需在客户映射地址池范围内</div>
          </div>

          <div class="flex gap-8 mt-16">
            <button class="btn btn-primary" :disabled="saving || !hasChange" @click="handleSave">
              {{ saving ? '保存中...' : '保存配置' }}
            </button>
            <button class="btn" :disabled="!hasChange" @click="handleReset">重置</button>
          </div>

          <div v-if="saveResult" class="alert mt-16" :class="saveResult.success ? 'alert-success' : 'alert-error'">
            {{ saveResult.message }}
          </div>
        </div>
      </div>
    </template>

    <ConfirmDialog
      :visible="confirmVisible"
      title="确认修改地址池"
      confirm-text="确认修改"
      confirm-type="danger"
      :content="'地址池已被使用，修改可能导致已分配地址失效并中断现有连接。\n\n确认要继续修改吗？'"
      @confirm="doSave"
      @cancel="confirmVisible = false"
    />
  </div>
</template>

<style scoped>
.pool-usage {
  padding: 8px 0;
}

.pool-usage-label {
  font-size: 13px;
  color: var(--text-secondary);
  margin-bottom: 6px;
}

.pool-usage-value {
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 8px;
}

.pool-usage-bar {
  height: 8px;
  background: var(--border-color-light);
  border-radius: 4px;
  overflow: hidden;
}

.pool-usage-fill {
  height: 100%;
  background: var(--color-primary);
  border-radius: 4px;
  transition: width 0.3s;
}
</style>
