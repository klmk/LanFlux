<script setup>
import { ref, computed, onMounted } from 'vue'
import { getMappings } from '../api.js'
import StatusBadge from '../components/StatusBadge.vue'

const loading = ref(true)
const mappings = ref([])
const keyword = ref('')
const copiedId = ref('')

const filteredMappings = computed(() => {
  if (!keyword.value) return mappings.value
  const kw = keyword.value.toLowerCase()
  return mappings.value.filter(
    (m) =>
      m.nodeName.toLowerCase().includes(kw) ||
      m.segmentName.toLowerCase().includes(kw) ||
      m.realCidr.toLowerCase().includes(kw) ||
      m.mappedCidr.toLowerCase().includes(kw)
  )
})

async function copyText(text, id) {
  try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      await navigator.clipboard.writeText(text)
    } else {
      // 兼容降级
      const ta = document.createElement('textarea')
      ta.value = text
      ta.style.position = 'fixed'
      ta.style.opacity = '0'
      document.body.appendChild(ta)
      ta.select()
      document.execCommand('copy')
      document.body.removeChild(ta)
    }
    copiedId.value = id
    setTimeout(() => {
      if (copiedId.value === id) copiedId.value = ''
    }, 1500)
  } catch (e) {
    console.error('复制失败', e)
  }
}

function buildFullMapping(m) {
  return [
    `客户/节点：${m.nodeName}`,
    `网段名称：${m.segmentName}`,
    `真实网段：${m.realCidr}`,
    `映射网段：${m.mappedCidr}`,
    `访问示例：${m.accessExample}`,
    `状态：${m.status === 'enabled' ? '已启用' : m.status === 'pending' ? '待确认' : m.status}`
  ].join('\n')
}

async function loadData() {
  loading.value = true
  try {
    mappings.value = await getMappings()
  } catch (e) {
    console.error('加载映射关系失败', e)
  } finally {
    loading.value = false
  }
}

onMounted(loadData)
</script>

<template>
  <div class="card">
    <!-- 搜索栏 -->
    <div class="toolbar">
      <div class="toolbar-item">
        <label>搜索</label>
        <input
          class="form-input"
          v-model="keyword"
          placeholder="客户/节点、网段名称、真实网段、映射网段"
          style="min-width: 320px"
        />
      </div>
      <div class="spacer"></div>
      <button class="btn" @click="keyword = ''">重置</button>
      <button class="btn" @click="loadData">刷新</button>
    </div>

    <!-- 表格 -->
    <div class="table-wrap">
      <table class="data-table">
        <thead>
          <tr>
            <th>客户 / 节点</th>
            <th>网段名称</th>
            <th>真实网段</th>
            <th>映射网段</th>
            <th>访问示例</th>
            <th>状态</th>
            <th>操作</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="loading">
            <td colspan="7" class="text-center text-muted">加载中...</td>
          </tr>
          <tr v-else-if="!filteredMappings.length">
            <td colspan="7" class="text-center text-muted">暂无数据</td>
          </tr>
          <tr v-for="m in filteredMappings" :key="m.id">
            <td>{{ m.nodeName }}</td>
            <td>{{ m.segmentName }}</td>
            <td>
              <span class="mono">{{ m.realCidr }}</span>
              <button class="copy-btn" title="复制真实网段" @click="copyText(m.realCidr, m.id + '-real')">复制</button>
            </td>
            <td>
              <span class="mono">{{ m.mappedCidr }}</span>
              <button class="copy-btn" title="复制映射网段" @click="copyText(m.mappedCidr, m.id + '-mapped')">复制</button>
            </td>
            <td>
              <span class="text-secondary">{{ m.accessExample }}</span>
              <button class="copy-btn" title="复制访问示例" @click="copyText(m.accessExample, m.id + '-example')">复制</button>
            </td>
            <td><StatusBadge :status="m.status" type="segment" /></td>
            <td class="actions-cell">
              <button class="btn-link" @click="copyText(buildFullMapping(m), m.id + '-full')">
                {{ copiedId === m.id + '-full' ? '已复制' : '复制完整说明' }}
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div v-if="copiedId" class="copy-toast">已复制到剪贴板</div>
  </div>
</template>

<style scoped>
.copy-btn {
  display: inline-block;
  margin-left: 6px;
  padding: 1px 6px;
  font-size: 11px;
  color: var(--color-primary);
  background: var(--color-primary-light);
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  vertical-align: middle;
}

.copy-btn:hover {
  background: var(--color-primary);
  color: #fff;
}

.copy-toast {
  position: fixed;
  bottom: 30px;
  left: 50%;
  transform: translateX(-50%);
  background: rgba(30, 41, 59, 0.9);
  color: #fff;
  padding: 8px 18px;
  border-radius: var(--radius);
  font-size: 13px;
  z-index: 999;
}
</style>
