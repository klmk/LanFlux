<script setup>
import { computed } from 'vue'

const props = defineProps({
  // 状态码
  status: {
    type: String,
    required: true
  },
  // 类型：node（节点状态）或 segment（网段状态）
  type: {
    type: String,
    default: 'node'
  }
})

// 节点状态映射
const nodeStatusMap = {
  disconnected: { label: '未连接', color: 'gray' },
  connecting: { label: '连接中', color: 'yellow' },
  connected: { label: '已连接', color: 'blue' },
  networked: { label: '已组网', color: 'green' },
  partial: { label: '部分异常', color: 'orange' },
  offline: { label: '已断开', color: 'red' }
}

// 网段状态映射
const segmentStatusMap = {
  pending: { label: '待确认', color: 'yellow' },
  enabled: { label: '已启用', color: 'green' },
  disabled: { label: '已停用', color: 'gray' },
  error: { label: '异常', color: 'red' }
}

const info = computed(() => {
  const map = props.type === 'segment' ? segmentStatusMap : nodeStatusMap
  return map[props.status] || { label: props.status, color: 'gray' }
})
</script>

<template>
  <span class="status-badge" :class="'status-' + info.color">{{ info.label }}</span>
</template>
