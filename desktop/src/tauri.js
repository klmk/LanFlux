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

const mockStatus = {
  mode: 'idle',
  connection_status: '已断开',
  tunnel_status: '已断开',
  node_id: null,
  node_name: null,
  virtual_ip: null,
  server_addr: null,
  accessible_segments_count: 0,
  reported_segments_count: 0
}

/* ------------------------------------------------------------------ *
 *  基础命令
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

/* ------------------------------------------------------------------ *
 *  网络连接命令
 * ------------------------------------------------------------------ */

/**
 * 以客户端模式连接服务端
 * @param {string} serverAddr 服务端地址
 * @param {string} nodeName 节点名称
 * @param {string|null} remark 备注
 * @returns {Promise<Object>} 连接结果 { node_id, virtual_ip, accessible_segments }
 */
export async function connectClient(serverAddr, nodeName, remark = null) {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('connect_client', { serverAddr, nodeName, remark })
  }
  // Mock
  return new Promise((resolve) => {
    setTimeout(() => resolve({
      node_id: 'mock-client-' + Date.now(),
      virtual_ip: null,
      accessible_segments: []
    }), 800)
  })
}

/**
 * 以实施端模式连接服务端
 * @param {string} serverAddr 服务端地址
 * @param {string} nodeName 节点名称
 * @returns {Promise<Object>} 连接结果 { node_id, virtual_ip, accessible_segments }
 */
export async function connectOperator(serverAddr, nodeName) {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('connect_operator', { serverAddr, nodeName })
  }
  // Mock
  return new Promise((resolve) => {
    setTimeout(() => resolve({
      node_id: 'mock-operator-' + Date.now(),
      virtual_ip: '100.127.0.2',
      accessible_segments: [
        { target_node_id: 'n1', target_node_name: '客户A-网关机', segment_name: '办公网', real_cidr: '192.168.1.0/24', mapped_cidr: '100.64.1.0/24' },
        { target_node_id: 'n2', target_node_name: '客户B-网关机', segment_name: '生产网', real_cidr: '192.168.2.0/24', mapped_cidr: '100.64.2.0/24' }
      ]
    }), 800)
  })
}

/**
 * 启动隧道
 * @returns {Promise<Object>} 隧道启动结果 { virtual_ip, routes }
 */
export async function startTunnel() {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('start_tunnel')
  }
  // Mock
  return new Promise((resolve) => {
    setTimeout(() => resolve({
      virtual_ip: '100.127.0.2',
      routes: [
        { target_node_id: 'n1', target_node_name: '客户A-网关机', segment_name: '办公网', real_cidr: '192.168.1.0/24', mapped_cidr: '100.64.1.0/24' }
      ]
    }), 1000)
  })
}

/**
 * 停止隧道
 * @returns {Promise<void>}
 */
export async function stopTunnel() {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('stop_tunnel')
  }
  return Promise.resolve()
}

/**
 * 断开所有连接
 * @returns {Promise<void>}
 */
export async function disconnectAll() {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('disconnect_all')
  }
  return Promise.resolve()
}

/**
 * 获取综合状态
 * @returns {Promise<Object>} 状态信息
 */
export async function getStatus() {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('get_status')
  }
  return Promise.resolve(JSON.parse(JSON.stringify(mockStatus)))
}

/**
 * 获取隧道路由
 * @returns {Promise<Array>} 路由列表
 */
export async function getTunnelRoutes() {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('get_tunnel_routes')
  }
  return Promise.resolve([])
}

/**
 * 获取可访问网段（实施端）
 * @returns {Promise<Array>} 网段列表
 */
export async function getAccessibleSegments() {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('get_accessible_segments')
  }
  return Promise.resolve([])
}

/**
 * 刷新可访问网段
 * @returns {Promise<Array>} 网段列表
 */
export async function refreshAccessibleSegments() {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('refresh_accessible_segments')
  }
  return Promise.resolve([])
}

/**
 * 获取已上报网段（客户端）
 * @returns {Promise<Array>} 网段列表
 */
export async function getReportedSegments() {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('get_reported_segments')
  }
  return Promise.resolve([])
}

/**
 * 刷新已上报网段
 * @returns {Promise<Array>} 网段列表
 */
export async function refreshReportedSegments() {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('refresh_reported_segments')
  }
  return Promise.resolve([])
}

/**
 * 上报网段
 * @param {string} name 网段名称
 * @param {string} realCidr 真实网段
 * @param {string|null} remark 备注
 * @returns {Promise<Object>} 上报结果
 */
export async function reportSegment(name, realCidr, remark = null) {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('report_segment', { name, realCidr, remark })
  }
  return new Promise((resolve) => {
    setTimeout(() => resolve({
      segment_id: 'mock-seg-' + Date.now(),
      real_cidr: realCidr,
      mapped_cidr: '100.64.' + Math.floor(Math.random() * 254 + 1) + '.0/24',
      status: 'active'
    }), 600)
  })
}

/**
 * Ping 测试
 * @param {string} targetIp 目标 IP
 * @returns {Promise<Object>} Ping 结果 { success, target, elapsed_ms, message }
 */
export async function pingTest(targetIp) {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('ping_test', { targetIp })
  }
  // Mock
  return new Promise((resolve) => {
    setTimeout(() => resolve({
      success: true,
      target: targetIp,
      elapsed_ms: Math.floor(Math.random() * 50) + 5,
      message: `4 packets transmitted, 4 received, 0% packet loss, time 3010ms\nrtt min/avg/max/mdev = 5.123/8.456/12.789/2.345 ms`
    }), 1000)
  })
}

/**
 * TCP 连通性测试
 * @param {string} targetIp 目标 IP
 * @param {number} port 端口
 * @returns {Promise<Object>} TCP 测试结果 { success, target, port, elapsed_ms, message }
 */
export async function tcpTest(targetIp, port) {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('tcp_test', { targetIp, port })
  }
  // Mock
  return new Promise((resolve) => {
    setTimeout(() => resolve({
      success: true,
      target: targetIp,
      port: port,
      elapsed_ms: Math.floor(Math.random() * 100) + 10,
      message: `成功连接 ${targetIp}:${port}`
    }), 800)
  })
}

/**
 * IP 转换
 * @param {string} inputIp 输入 IP
 * @returns {Promise<Object>} 转换结果 { input_ip, real_cidr, mapped_cidr, mapped_ip, message }
 */
export async function convertIp(inputIp) {
  const invoke = await getInvoke()
  if (invoke) {
    return invoke('convert_ip', { inputIp })
  }
  // Mock
  return new Promise((resolve) => {
    setTimeout(() => {
      if (inputIp.startsWith('192.168.1.')) {
        const last = inputIp.split('.').pop()
        resolve({
          input_ip: inputIp,
          real_cidr: '192.168.1.0/24',
          mapped_cidr: '100.64.1.0/24',
          mapped_ip: '100.64.1.' + last,
          message: `${inputIp} (192.168.1.0/24) -> 100.64.1.${last} (100.64.1.0/24)`
        })
      } else {
        resolve({
          input_ip: inputIp,
          real_cidr: null,
          mapped_cidr: null,
          mapped_ip: null,
          message: `IP ${inputIp} 不在任何已知网段中`
        })
      }
    }, 300)
  })
}

export default {
  // 基础命令
  scanInterfaces,
  getAppInfo,
  testConnectivity,
  saveConfig,
  loadConfig,
  // 网络连接命令
  connectClient,
  connectOperator,
  startTunnel,
  stopTunnel,
  disconnectAll,
  getStatus,
  getTunnelRoutes,
  getAccessibleSegments,
  refreshAccessibleSegments,
  getReportedSegments,
  refreshReportedSegments,
  reportSegment,
  pingTest,
  tcpTest,
  convertIp
}
