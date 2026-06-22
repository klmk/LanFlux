import { createRouter, createWebHashHistory } from 'vue-router'

import ModeSelect from './views/ModeSelect.vue'
import ClientHome from './views/ClientHome.vue'
import ClientSegments from './views/ClientSegments.vue'
import ClientStatus from './views/ClientStatus.vue'
import OperatorHome from './views/OperatorHome.vue'
import OperatorSegments from './views/OperatorSegments.vue'
import OperatorTest from './views/OperatorTest.vue'
import ServerStatus from './views/ServerStatus.vue'

const routes = [
  { path: '/', name: 'ModeSelect', component: ModeSelect, meta: { title: '模式选择' } },
  { path: '/client', name: 'ClientHome', component: ClientHome, meta: { title: '客户端' } },
  { path: '/client/segments', name: 'ClientSegments', component: ClientSegments, meta: { title: '网段管理' } },
  { path: '/client/status', name: 'ClientStatus', component: ClientStatus, meta: { title: '连接状态' } },
  { path: '/operator', name: 'OperatorHome', component: OperatorHome, meta: { title: '实施端' } },
  { path: '/operator/segments', name: 'OperatorSegments', component: OperatorSegments, meta: { title: '可访问网段' } },
  { path: '/operator/test', name: 'OperatorTest', component: OperatorTest, meta: { title: '连通性测试' } },
  { path: '/server', name: 'ServerStatus', component: ServerStatus, meta: { title: '服务端状态' } }
]

const router = createRouter({
  history: createWebHashHistory(),
  routes
})

export default router
