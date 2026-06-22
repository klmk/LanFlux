import axios from 'axios'

const client = axios.create({
  baseURL: '/api/v1',
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

/* ------------------------------------------------------------------ *
 *  Mock 数据（当后端不可用时自动回退，便于前端独立开发与演示）
 * ------------------------------------------------------------------ */

const mockNodes = [
  { id: 'n-001', name: '北京总部节点', role: 'client', status: 'networked', os: 'Linux Ubuntu 22.04', virtualIp: '10.10.0.2', segmentCount: 4, lastOnline: '2026-06-21 09:12:33', remark: '总部办公网' },
  { id: 'n-002', name: '上海分支节点', role: 'client', status: 'connected', os: 'Windows Server 2019', virtualIp: '10.10.0.3', segmentCount: 2, lastOnline: '2026-06-21 09:15:01', remark: '上海分公司' },
  { id: 'n-003', name: '广州实施端-A', role: 'agent', status: 'connected', os: 'Linux CentOS 7', virtualIp: '10.20.0.10', segmentCount: 0, lastOnline: '2026-06-21 09:14:50', remark: '广州机房实施端' },
  { id: 'n-004', name: '深圳研发节点', role: 'client', status: 'partial', os: 'Linux Debian 11', virtualIp: '10.10.0.4', segmentCount: 1, lastOnline: '2026-06-21 08:50:22', remark: '深圳研发中心' },
  { id: 'n-005', name: '成都实施端-B', role: 'agent', status: 'disconnected', os: 'Linux Ubuntu 20.04', virtualIp: '10.20.0.11', segmentCount: 0, lastOnline: '2026-06-20 22:31:10', remark: '成都机房实施端' },
  { id: 'n-006', name: '杭州办事处节点', role: 'client', status: 'connecting', os: 'Windows 10', virtualIp: '10.10.0.5', segmentCount: 2, lastOnline: '2026-06-21 09:10:00', remark: '杭州办事处' },
  { id: 'n-007', name: '武汉分支节点', role: 'client', status: 'networked', os: 'Linux Ubuntu 22.04', virtualIp: '10.10.0.6', segmentCount: 3, lastOnline: '2026-06-21 09:16:20', remark: '武汉分公司' },
  { id: 'n-008', name: '西安实施端-C', role: 'agent', status: 'connected', os: 'Linux Rocky 9', virtualIp: '10.20.0.12', segmentCount: 0, lastOnline: '2026-06-21 09:13:05', remark: '西安机房实施端' },
  { id: 'n-009', name: '南京办公节点', role: 'client', status: 'offline', os: 'Windows Server 2022', virtualIp: '10.10.0.7', segmentCount: 1, lastOnline: '2026-06-19 18:02:45', remark: '南京办事处' },
  { id: 'n-010', name: '重庆分支节点', role: 'client', status: 'connected', os: 'Linux Ubuntu 22.04', virtualIp: '10.10.0.8', segmentCount: 2, lastOnline: '2026-06-21 09:11:30', remark: '重庆分公司' }
]

const mockSegments = [
  { id: 's-001', name: '北京办公网段', nodeId: 'n-001', nodeName: '北京总部节点', realCidr: '192.168.10.0/24', mappedCidr: '172.16.1.0/24', status: 'enabled', conflict: false, remark: '总部办公终端网段' },
  { id: 's-002', name: '北京服务器网段', nodeId: 'n-001', nodeName: '北京总部节点', realCidr: '192.168.20.0/24', mappedCidr: '172.16.2.0/24', status: 'enabled', conflict: false, remark: '总部服务器网段' },
  { id: 's-003', name: '上海办公网段', nodeId: 'n-002', nodeName: '上海分支节点', realCidr: '192.168.30.0/24', mappedCidr: '172.16.3.0/24', status: 'enabled', conflict: false, remark: '上海办公终端' },
  { id: 's-004', name: '上海监控网段', nodeId: 'n-002', nodeName: '上海分支节点', realCidr: '192.168.30.0/24', mappedCidr: '172.16.4.0/24', status: 'error', conflict: true, remark: '与北京办公网段真实地址冲突' },
  { id: 's-005', name: '深圳研发网段', nodeId: 'n-004', nodeName: '深圳研发节点', realCidr: '10.0.0.0/24', mappedCidr: '172.16.5.0/24', status: 'pending', conflict: false, remark: '待确认启用' },
  { id: 's-006', name: '杭州办事处网段', nodeId: 'n-006', nodeName: '杭州办事处节点', realCidr: '192.168.50.0/24', mappedCidr: '172.16.6.0/24', status: 'enabled', conflict: false, remark: '杭州办公网段' },
  { id: 's-007', name: '武汉办公网段', nodeId: 'n-007', nodeName: '武汉分支节点', realCidr: '192.168.60.0/24', mappedCidr: '172.16.7.0/24', status: 'enabled', conflict: false, remark: '武汉办公终端' },
  { id: 's-008', name: '武汉生产网段', nodeId: 'n-007', nodeName: '武汉分支节点', realCidr: '192.168.61.0/24', mappedCidr: '172.16.8.0/24', status: 'disabled', conflict: false, remark: '临时停用' },
  { id: 's-009', name: '南京办公网段', nodeId: 'n-009', nodeName: '南京办公节点', realCidr: '192.168.70.0/24', mappedCidr: '172.16.9.0/24', status: 'disabled', conflict: false, remark: '节点离线，已停用' },
  { id: 's-010', name: '重庆办公网段', nodeId: 'n-010', nodeName: '重庆分支节点', realCidr: '192.168.80.0/24', mappedCidr: '172.16.10.0/24', status: 'enabled', conflict: false, remark: '重庆办公终端' }
]

const mockMappings = [
  { id: 'm-001', nodeName: '北京总部节点', segmentName: '北京办公网段', realCidr: '192.168.10.0/24', mappedCidr: '172.16.1.0/24', accessExample: '访问 172.16.1.0/24 即转发至北京办公网段', status: 'enabled' },
  { id: 'm-002', nodeName: '北京总部节点', segmentName: '北京服务器网段', realCidr: '192.168.20.0/24', mappedCidr: '172.16.2.0/24', accessExample: '访问 172.16.2.0/24 即转发至北京服务器网段', status: 'enabled' },
  { id: 'm-003', nodeName: '上海分支节点', segmentName: '上海办公网段', realCidr: '192.168.30.0/24', mappedCidr: '172.16.3.0/24', accessExample: '访问 172.16.3.0/24 即转发至上海办公网段', status: 'enabled' },
  { id: 'm-004', nodeName: '深圳研发节点', segmentName: '深圳研发网段', realCidr: '10.0.0.0/24', mappedCidr: '172.16.5.0/24', accessExample: '访问 172.16.5.0/24 即转发至深圳研发网段', status: 'pending' },
  { id: 'm-005', nodeName: '杭州办事处节点', segmentName: '杭州办事处网段', realCidr: '192.168.50.0/24', mappedCidr: '172.16.6.0/24', accessExample: '访问 172.16.6.0/24 即转发至杭州办事处网段', status: 'enabled' },
  { id: 'm-006', nodeName: '武汉分支节点', segmentName: '武汉办公网段', realCidr: '192.168.60.0/24', mappedCidr: '172.16.7.0/24', accessExample: '访问 172.16.7.0/24 即转发至武汉办公网段', status: 'enabled' },
  { id: 'm-007', nodeName: '重庆分支节点', segmentName: '重庆办公网段', realCidr: '192.168.80.0/24', mappedCidr: '172.16.10.0/24', accessExample: '访问 172.16.10.0/24 即转发至重庆办公网段', status: 'enabled' }
]

const mockPolicies = [
  { nodeId: 'n-001', nodeName: '北京总部节点', accessMode: 'all', allowedSegments: [], selectedSegments: [] },
  { nodeId: 'n-002', nodeName: '上海分支节点', accessMode: 'specified', allowedSegments: ['s-001', 's-002', 's-006', 's-007'], selectedSegments: ['s-001', 's-002'] },
  { nodeId: 'n-004', nodeName: '深圳研发节点', accessMode: 'specified', allowedSegments: ['s-001', 's-003', 's-007'], selectedSegments: ['s-001'] },
  { nodeId: 'n-006', nodeName: '杭州办事处节点', accessMode: 'specified', allowedSegments: ['s-001', 's-002', 's-003', 's-007'], selectedSegments: ['s-001', 's-003', 's-007'] },
  { nodeId: 'n-007', nodeName: '武汉分支节点', accessMode: 'deny', allowedSegments: ['s-001', 's-002', 's-003'], selectedSegments: [] },
  { nodeId: 'n-010', nodeName: '重庆分支节点', accessMode: 'specified', allowedSegments: ['s-001', 's-002', 's-003', 's-006'], selectedSegments: ['s-001', 's-006'] }
]

const mockPools = {
  clientPool: '10.10.0.0/16',
  segmentSize: 24,
  agentPool: '10.20.0.0/16',
  serverVirtualIp: '10.10.0.1',
  clientPoolUsed: 8,
  clientPoolTotal: 254,
  agentPoolUsed: 3,
  agentPoolTotal: 254
}

const mockSessions = [
  { id: 'sess-001', sourceNode: '北京总部节点', targetSegment: '上海办公网段', protocol: 'TCP', targetAddress: '172.16.3.25:8080', customer: '北京总部', startTime: '2026-06-21 08:30:12', lastActivity: '2026-06-21 09:15:40', status: 'active', timeout: '' },
  { id: 'sess-002', sourceNode: '上海分支节点', targetSegment: '北京服务器网段', protocol: 'TCP', targetAddress: '172.16.2.10:443', customer: '上海分公司', startTime: '2026-06-21 09:01:05', lastActivity: '2026-06-21 09:16:01', status: 'active', timeout: '' },
  { id: 'sess-003', sourceNode: '杭州办事处节点', targetSegment: '北京办公网段', protocol: 'UDP', targetAddress: '172.16.1.55:53', customer: '杭州办事处', startTime: '2026-06-21 09:10:00', lastActivity: '2026-06-21 09:16:30', status: 'active', timeout: '300s' },
  { id: 'sess-004', sourceNode: '武汉分支节点', targetSegment: '上海办公网段', protocol: 'TCP', targetAddress: '172.16.3.20:22', customer: '武汉分公司', startTime: '2026-06-21 08:45:33', lastActivity: '2026-06-21 09:05:10', status: 'idle', timeout: '' },
  { id: 'sess-005', sourceNode: '重庆分支节点', targetSegment: '北京办公网段', protocol: 'UDP', targetAddress: '172.16.1.100:5060', customer: '重庆分公司', startTime: '2026-06-21 09:12:00', lastActivity: '2026-06-21 09:14:55', status: 'active', timeout: '120s' },
  { id: 'sess-006', sourceNode: '深圳研发节点', targetSegment: '北京服务器网段', protocol: 'TCP', targetAddress: '172.16.2.20:3306', customer: '深圳研发中心', startTime: '2026-06-21 07:20:00', lastActivity: '2026-06-21 08:50:00', status: 'closed', timeout: '' }
]

const mockLogs = [
  { id: 'l-001', category: 'system', level: 'info', time: '2026-06-21 09:16:20', node: '-', message: '服务端心跳正常，当前在线节点 7 个' },
  { id: 'l-002', category: 'node', level: 'info', time: '2026-06-21 09:15:01', node: '上海分支节点', message: '节点建立连接，虚拟 IP 10.10.0.3' },
  { id: 'l-003', category: 'segment', level: 'warning', time: '2026-06-21 09:14:50', node: '广州实施端-A', message: '网段上报：上海监控网段真实地址与北京办公网段冲突' },
  { id: 'l-004', category: 'route', level: 'info', time: '2026-06-21 09:13:05', node: '西安实施端-C', message: '路由下发成功，共下发 6 条映射路由' },
  { id: 'l-005', category: 'policy', level: 'info', time: '2026-06-21 09:10:30', node: '杭州办事处节点', message: '权限变更：访问模式由「指定网段」变更为「指定网段」，新增可访问网段 武汉办公网段' },
  { id: 'l-006', category: 'forward', level: 'error', time: '2026-06-21 09:08:12', node: '深圳研发节点', message: '转发异常：目标网段 172.16.2.0/24 不可达，部分会话中断' },
  { id: 'l-007', category: 'node', level: 'error', time: '2026-06-20 22:31:10', node: '成都实施端-B', message: '节点连接断开，原因：心跳超时' },
  { id: 'l-008', category: 'system', level: 'warning', time: '2026-06-20 22:31:15', node: '-', message: '实施端在线数量低于阈值（当前 2，阈值 3）' },
  { id: 'l-009', category: 'segment', level: 'info', time: '2026-06-21 08:50:22', node: '深圳研发节点', message: '网段上报：深圳研发网段 10.0.0.0/24，待确认' },
  { id: 'l-010', category: 'route', level: 'warning', time: '2026-06-21 09:00:00', node: '南京办公节点', message: '路由下发失败：节点离线，将在节点重连后重试' }
]

const mockDashboard = {
  stats: {
    onlineNodes: 7,
    registeredSegments: 10,
    agentOnline: 3,
    alerts: 2
  },
  recentNodes: mockNodes.slice(0, 5),
  recentAlerts: mockLogs.filter((l) => l.level === 'error' || l.level === 'warning').slice(0, 6),
  mappingOverview: mockSegments.filter((s) => s.status === 'enabled').slice(0, 8)
}

// 当请求失败时回退到 Mock 数据
function fallback(mockData) {
  return (err) => {
    console.warn('[api] 请求失败，使用 Mock 数据:', err?.message || err)
    return Promise.resolve(JSON.parse(JSON.stringify(mockData)))
  }
}

/* ------------------------------------------------------------------ *
 *  Dashboard
 * ------------------------------------------------------------------ */
export function getDashboard() {
  return client.get('/dashboard').catch(fallback(mockDashboard))
}

/* ------------------------------------------------------------------ *
 *  Nodes
 * ------------------------------------------------------------------ */
export function getNodes(params) {
  return client.get('/nodes', { params }).catch(fallback(mockNodes))
}

export function getNode(id) {
  const found = mockNodes.find((n) => n.id === id) || mockNodes[0]
  return client.get(`/nodes/${id}`).catch(fallback({
    ...found,
    connection: {
      connectTime: '2026-06-21 08:30:00',
      remoteAddr: '114.114.114.114:51820',
      protocol: 'WireGuard',
      rxBytes: 1024000,
      txBytes: 2048000,
      latency: 32
    },
    reportedSegments: mockSegments.filter((s) => s.nodeId === found.id),
    accessibleSegments: mockSegments.filter((s) => s.status === 'enabled' && s.nodeId !== found.id).slice(0, 5),
    routeStatus: 'synced',
    routeDetail: { total: 8, synced: 8, failed: 0 },
    recentLogs: mockLogs.filter((l) => l.node === found.name || l.node === '-').slice(0, 5)
  }))
}

export function updateNode(id, data) {
  return client.put(`/nodes/${id}`, data).catch(fallback({ success: true }))
}

export function disableNode(id) {
  return client.post(`/nodes/${id}/disable`).catch(fallback({ success: true }))
}

export function enableNode(id) {
  return client.post(`/nodes/${id}/enable`).catch(fallback({ success: true }))
}

export function kickNode(id) {
  return client.post(`/nodes/${id}/kick`).catch(fallback({ success: true }))
}

/* ------------------------------------------------------------------ *
 *  Segments
 * ------------------------------------------------------------------ */
export function getSegments(params) {
  return client.get('/segments', { params }).catch(fallback(mockSegments))
}

export function getSegment(id) {
  const found = mockSegments.find((s) => s.id === id) || mockSegments[0]
  return client.get(`/segments/${id}`).catch(fallback(found))
}

export function updateSegment(id, data) {
  return client.put(`/segments/${id}`, data).catch(fallback({ success: true }))
}

export function enableSegment(id) {
  return client.post(`/segments/${id}/enable`).catch(fallback({ success: true }))
}

export function disableSegment(id) {
  return client.post(`/segments/${id}/disable`).catch(fallback({ success: true }))
}

export function remapSegment(id) {
  return client.post(`/segments/${id}/remap`).catch(fallback({ success: true, newMappedCidr: '172.16.99.0/24' }))
}

/* ------------------------------------------------------------------ *
 *  Mappings
 * ------------------------------------------------------------------ */
export function getMappings() {
  return client.get('/mappings').catch(fallback(mockMappings))
}

/* ------------------------------------------------------------------ *
 *  Policies
 * ------------------------------------------------------------------ */
export function getPolicies() {
  return client.get('/policies').catch(fallback({
    nodes: mockNodes.filter((n) => n.role === 'client'),
    policies: mockPolicies,
    segments: mockSegments
  }))
}

export function getPolicyByNode(nodeId) {
  const found = mockPolicies.find((p) => p.nodeId === nodeId) || mockPolicies[0]
  return client.get(`/policies/${nodeId}`).catch(fallback({
    ...found,
    allSegments: mockSegments
  }))
}

export function updatePolicy(nodeId, data) {
  return client.put(`/policies/${nodeId}`, data).catch(fallback({ success: true, pushed: true, routeCount: 5 }))
}

/* ------------------------------------------------------------------ *
 *  Pools
 * ------------------------------------------------------------------ */
export function getPools() {
  return client.get('/pools').catch(fallback(mockPools))
}

export function updatePools(data) {
  return client.put('/pools', data).catch(fallback({ success: true }))
}

/* ------------------------------------------------------------------ *
 *  Sessions
 * ------------------------------------------------------------------ */
export function getSessions() {
  return client.get('/sessions').catch(fallback(mockSessions))
}

/* ------------------------------------------------------------------ *
 *  Logs
 * ------------------------------------------------------------------ */
export function getLogs() {
  return client.get('/logs').catch(fallback(mockLogs))
}

export default {
  getDashboard,
  getNodes, getNode, updateNode, disableNode, enableNode, kickNode,
  getSegments, getSegment, updateSegment, enableSegment, disableSegment, remapSegment,
  getMappings,
  getPolicies, getPolicyByNode, updatePolicy,
  getPools, updatePools,
  getSessions,
  getLogs
}
