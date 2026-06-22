import { createRouter, createWebHashHistory } from 'vue-router'

import Dashboard from './views/Dashboard.vue'
import NodeList from './views/NodeList.vue'
import NodeDetail from './views/NodeDetail.vue'
import SegmentList from './views/SegmentList.vue'
import MappingList from './views/MappingList.vue'
import PolicyList from './views/PolicyList.vue'
import PoolConfig from './views/PoolConfig.vue'
import SessionList from './views/SessionList.vue'
import LogList from './views/LogList.vue'

const routes = [
  { path: '/', name: 'Dashboard', component: Dashboard, meta: { title: '仪表盘' } },
  { path: '/nodes', name: 'NodeList', component: NodeList, meta: { title: '节点管理' } },
  { path: '/nodes/:id', name: 'NodeDetail', component: NodeDetail, meta: { title: '节点详情' } },
  { path: '/segments', name: 'SegmentList', component: SegmentList, meta: { title: '网段管理' } },
  { path: '/mappings', name: 'MappingList', component: MappingList, meta: { title: '映射关系' } },
  { path: '/policies', name: 'PolicyList', component: PolicyList, meta: { title: '权限配置' } },
  { path: '/pools', name: 'PoolConfig', component: PoolConfig, meta: { title: '地址池配置' } },
  { path: '/sessions', name: 'SessionList', component: SessionList, meta: { title: '连接会话' } },
  { path: '/logs', name: 'LogList', component: LogList, meta: { title: '日志诊断' } }
]

const router = createRouter({
  history: createWebHashHistory(),
  routes
})

export default router
