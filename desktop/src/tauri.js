/**
 * Tauri API 封装
 *
 * 调用 @tauri-apps/api 的 invoke 调用 Rust 后端命令。
 * 在非 Tauri 环境（浏览器调试）下提供 Mock 回退。
 */

// 检测是否在 Tauri 环境中
const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

// 懒加载 invoke 函数
let _invoke = null
async function getInvoke() {
  if (_invoke !== null) return _invoke
  if (isTauri) {
    try {
      const tauriApi = await import('@tauri-apps/api/core')
      _invoke = tauriApi.invoke
    } catch {
      _invoke = null
    }
  }
  return _invoke
}

/* ------------------------------------------------------------------ *
 *  Mock 数据（浏览器调试用）
 * ------------------------------------------------------------------ */

const mockInterfaces = [
  { name: 'eth0', iface_type: '物理网卡', ip_address: '192.168.1.100', cidr: '192.168.1.0/24', gateway: '192.168.1.1', recommended: true },
  { name: 'wlan0', iface_type: 'Wi-Fi', ip_address: '192.168.31.50', cidr: '192.168.31.0/24', gateway: '192.168.31.1', recommended: true },
  { name: 'docker0', iface_type: 'Docker', ip_address: '172.17.0.1', cidr: '172.17.0.0/16', gateway: null, recommended: false },
  { name: 'vmnet8', iface_type: 'VMware', ip_address: '192.168.88.1', cidr: '192.168.88.0/24', gateway: null, recommended: false }
]

const mockAppInfo = {
  name: 'NetTool 组网工具',
  version: '0.1.0',
  os: 'linux',
  arch: 'x86_64'
}

/* ------------------------------------------------------------------ *
 *  Tauri 命令封装
 * ------------------------------------------------------------------ */

/**
 * 扫描本机网卡
 * @returns {Promise<Array>} 网卡列表
 */
export async function scanInterfaces() {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('scan_interfaces')
  }
  // Mock 回退
  return new Promise((resolve) => {
    setTimeout(() => resolve(JSON.parse(JSON.stringify(mockInterfaces))), 300)
  })
}

/**
 * 获取应用信息
 * @returns {Promise<Object>} 应用信息
 */
export async function getAppInfo() {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('get_app_info')
  }
  return Promise.resolve(JSON.parse(JSON.stringify(mockAppInfo)))
}

/**
 * 测试服务端连通性
 * @param {string} host 主机地址
 * @param {number} port 端口
 * @returns {Promise<Object>} 连通性测试结果
 */
export async function testConnectivity(host, port) {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('test_connectivity', { host, port })
  }
  // Mock 回退
  return new Promise((resolve) => {
    setTimeout(() => {
      resolve({
        success: true,
        elapsed_ms: Math.floor(Math.random() * 50) + 5,
        message: `成功连接 ${host}:${port}`
      })
    }, 500)
  })
}

/**
 * 保存配置到本地文件
 * @param {Object} config 配置对象
 * @returns {Promise<string>} 配置文件路径
 */
export async function saveConfig(config) {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('save_config', { config })
  }
  // Mock：保存到 localStorage
  localStorage.setItem('net-tool-config', JSON.stringify(config))
  return Promise.resolve('localStorage://net-tool-config')
}

/**
 * 从本地文件加载配置
 * @returns {Promise<Object>} 配置对象
 */
export async function loadConfig() {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('load_config')
  }
  // Mock：从 localStorage 读取
  const saved = localStorage.getItem('net-tool-config')
  if (saved) {
    try {
      return Promise.resolve(JSON.parse(saved))
    } catch {
      // fallthrough
    }
  }
  return Promise.resolve({
    mode: '',
    server_addr: '127.0.0.1:8443',
    node_name: '',
    remark: '',
    auto_reconnect: true
  })
}

export default {
  scanInterfaces,
  getAppInfo,
  testConnectivity,
  saveConfig,
  loadConfig
}
