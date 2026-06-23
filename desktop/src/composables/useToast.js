import { ref } from 'vue'

/**
 * 全局 Toast 状态管理（模块级单例）
 *
 * 全应用共享同一份 toast 列表，由 App.vue 中的 <ToastContainer /> 统一渲染。
 * 任意视图调用 showToast 即可弹出提示，无需各自挂载容器。
 */
const toasts = ref([])

// 模块级自增 ID，保证每条 toast 唯一
let _id = 0

/**
 * 移除指定 toast
 * @param {number} id toast id
 */
function remove(id) {
  toasts.value = toasts.value.filter((t) => t.id !== id)
}

// 别名，保持向后兼容
const removeToast = remove

/**
 * 弹出一条提示
 * @param {string} message 提示内容
 * @param {'success'|'error'|'warning'|'info'} type 类型
 * @param {number} duration 自动消失时长（ms），0 表示不自动消失
 * @returns {number} toast id
 */
function showToast(message, type = 'info', duration = 3000) {
  const id = ++_id
  toasts.value.push({ id, type, message })
  if (duration > 0) {
    setTimeout(() => remove(id), duration)
  }
  return id
}

/**
 * 获取全局 toast 控制句柄
 * @returns {{ toasts: import('vue').Ref<Array>, showToast: Function, remove: Function, removeToast: Function }}
 */
export function useToast() {
  return { toasts, showToast, remove, removeToast }
}
