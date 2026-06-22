/**
 * API 客户端
 *
 * 用于客户端/实施端与服务端 REST API 通信。
 * 当服务端不可用时自动回退到 Mock 数据，便于独立开发与演示。
 */

import axios from 'axios'

let baseURL = '/api/v1'

// 从 localStorage 读取上次保存的服务端地址
try {
  const saved = localStorage.getItem('net-tool-server-addr')
  if (saved) {
    baseURL = `http://${saved}/api/v1`
  }
} catch {
  // ignore
}

const client = axios.create({
  baseURL,
  timeout: 15000,
  headers: {
    'Content-Type': 'application/json'
  }
})

// 响应拦截器：统一提取 data
client.interceptors.response.use(
  (response) => response.data,
  (error) => Promise.reject(error)
)

/**
 * 更新服务端地址（连接时调用）
 * @param {string} serverAddr 服务端地址，如 192.168.1.10:8443
 */
export function setServerAddr(serverAddr) {
  const addr = serverAddr
    .trim()
    .replace(/^https?:\/\//, '')
    .replace(/\/$/, '')
  baseURL = `http://${addr}/api/v1`
  client.defaults.baseURL = baseURL
  try {
    localStorage.setItem('net-tool-server-addr', addr)
  } catch {
    // ignore
  }
}

/* ------------------------------------------------------------------ *
 *  Mock 数据
 * ------------------------------------------------------------------ */

const mockRegisterResponse = {
  node_id: 'desktop-001',
  virtual_ip: null
}

const mockOperatorRegisterResponse = {
  node_id: 'desktop-op-001',
  virtual_ip: '10.20.0.15'
}

const mockClientSegments = [
  { id: 's-001', name: '办公网段', real_cidr: '192.168.1.0/24', mapped_cidr: '172.16.1.0/24', status: 'active', remark: '办公终端网段' },
  { id: 's-002', name: '服务器网段', real_cidr: '192.168.20.0/24', mapped_cidr: '172.16.2.0/24', status: 'active', remark: '内部服务器网段' },
  { id: 's-003', name: '监控网段', real_cidr: '192.168.30.0/24', mapped_cidr: '172.16.3.0/24', status: 'pending', remark: '待确认' }
]

const mockOperatorSegments = [
  { segment_id: 's-001', node_id: 'n-001', node_name: '北京总部节点', segment_name: '北京办公网段', real_cidr: '192.168.10.0/24', mapped_cidr: '172.16.1.0/24', status: 'active' },
  { segment_id: 's-002', node_id: 'n-001', node_name: '北京总部节点', segment_name: '北京服务器网段', real_cidr: '192.168.20.0/24', mapped_cidr: '172.16.2.0/24', status: 'active' },
  { segment_id: 's-003', node_id: 'n-002', node_name: '上海分支节点', segment_name: '上海办公网段', real_cidr: '192.168.30.0/24', mapped_cidr: '172.16.3.0/24', status: 'active' },
  { segment_id: 's-004', node_id: 'n-004', node_name: '深圳研发节点', segment_name: '深圳研发网段', real_cidr: '10.0.0.0/24', mapped_cidr: '172.16.5.0/24', status: 'pending' },
  { segment_id: 's-005', node_id: 'n-006', node_name: '杭州办事处节点', segment_name: '杭州办事处网段', real_cidr: '192.168.50.0/24', mapped_cidr: '172.16.6.0/24', status: 'active' },
  { segment_id: 's-006', node_id: 'n-007', node_name: '武汉分支节点', segment_name: '武汉办公网段', real_cidr: '192.168.60.0/24', mapped_cidr: '172.16.7.0/24', status: 'active' }
]

const mockHeartbeatResponse = {
  routes: [
    { mapped_cidr: '172.16.1.0/24', target_node_id: 'n-001', target_node_name: '北京总部节点', real_cidr: '192.168.10.0/24', segment_name: '北京办公网段' },
    { mapped_cidr: '172.16.3.0/24', target_node_id: 'n-002', target_node_name: '上海分支节点', real_cidr: '192.168.30.0/24', segment_name: '上海办公网段' },
    { mapped_cidr: '172.16.6.0/24', target_node_id: 'n-006', target_node_name: '杭州办事处节点', real_cidr: '192.168.50.0/24', segment_name: '杭州办事处网段' }
  ]
}

const mockServerStats = {
  online_node_count: 7,
  registered_segment_count: 10,
  client_count: 5,
  operator_count: 3
}

// 当请求失败时回退到 Mock 数据
function fallback(mockData) {
  return (err) => {
    console.warn('[api] 请求失败，使用 Mock 数据:', err?.message || err)
    return Promise.resolve(JSON.parse(JSON.stringify(mockData)))
  }
}

/* ------------------------------------------------------------------ *
 *  节点注册 / 心跳
 * ------------------------------------------------------------------ */

/**
 * 注册节点
 * @param {Object} data { name, role, os_type, remark }
 */
export function registerNode(data) {
  const mock = data.role === 'operator' ? mockOperatorRegisterResponse : mockRegisterResponse
  return client.post('/nodes/register', data).catch(fallback(mock))
}

/**
 * 发送心跳
 * @param {Object} data { node_id, reported_segments_count }
 */
export function sendHeartbeat(data) {
  return client.post('/nodes/heartbeat', data).catch(fallback(mockHeartbeatResponse))
}

/* ------------------------------------------------------------------ *
 *  网段上报 / 查询
 * ------------------------------------------------------------------ */

/**
 * 上报网段
 * @param {Object} data { node_id, name, real_cidr, remark }
 */
export function reportSegment(data) {
  return client.post('/segments/report', data).catch(
    fallback({
      segment_id: 's-new-' + Date.now(),
      mapped_cidr: '172.16.99.0/24'
    })
  )
}

/**
 * 查询节点已上报网段
 * @param {string} nodeId 节点 ID
 */
export function querySegments(nodeId) {
  return client.get('/segments', { params: { node_id: nodeId } }).catch(fallback(mockClientSegments))
}

/* ------------------------------------------------------------------ *
 *  实施端 - 申请虚拟 IP / 查询可访问网段
 * ------------------------------------------------------------------ */

/**
 * 查询访问权限（可访问网段列表）
 * @param {string} nodeId 节点 ID
 */
export function queryAccess(nodeId) {
  return client
    .get('/access', { params: { node_id: nodeId } })
    .catch(
      fallback({
        access_mode: 'allowed_all',
        allowed_segments: mockOperatorSegments.map((s) => ({
          mapped_cidr: s.mapped_cidr,
          target_node_id: s.node_id,
          target_node_name: s.node_name,
          real_cidr: s.real_cidr,
          segment_name: s.segment_name
        }))
      })
    )
}

/* ------------------------------------------------------------------ *
 *  服务端状态
 * ------------------------------------------------------------------ */

/**
 * 获取仪表盘统计（服务端模式用）
 */
export function getDashboard() {
  return client.get('/dashboard').catch(fallback(mockServerStats))
}

export default {
  setServerAddr,
  registerNode,
  sendHeartbeat,
  reportSegment,
  querySegments,
  queryAccess,
  getDashboard
}
