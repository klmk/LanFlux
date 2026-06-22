<script setup>
import { useRouter } from 'vue-router'
import { saveConfig } from '../tauri.js'

const router = useRouter()

const modes = [
  {
    key: 'server',
    title: '服务端',
    icon: 'S',
    iconClass: 'icon-server',
    desc: '运行组网服务端，管理所有节点与网段映射。提供 Web 管理后台。',
    features: ['节点管理', '网段映射分配', '权限策略配置', 'Web 管理后台'],
    route: '/server'
  },
  {
    key: 'client',
    title: '普通客户端',
    icon: 'C',
    iconClass: 'icon-client',
    desc: '连接服务端，上报本地网段，供其他节点访问。适合需要共享本地网络的场景。',
    features: ['连接服务端', '上报本地网段', '接收路由下发', '自动重连'],
    route: '/client'
  },
  {
    key: 'operator',
    title: '实施端',
    icon: 'O',
    iconClass: 'icon-operator',
    desc: '连接服务端，获取虚拟 IP，访问其他节点上报的网段。适合运维实施人员。',
    features: ['获取虚拟 IP', '访问远程网段', '连通性测试', 'IP 换算工具'],
    route: '/operator'
  }
]

async function selectMode(mode) {
  // 保存模式到配置
  try {
    await saveConfig({
      mode: mode.key,
      server_addr: '127.0.0.1:8443',
      node_name: '',
      remark: '',
      auto_reconnect: true
    })
  } catch (e) {
    console.warn('保存配置失败', e)
  }

  // 通知 App 更新当前模式（通过路由跳转触发）
  router.push(mode.route).then(() => {
    // 刷新页面以让 App.vue 重新加载配置
    window.location.reload()
  })
}
</script>

<template>
  <div class="mode-select">
    <div class="mode-header">
      <h1 class="mode-title">选择运行模式</h1>
      <p class="mode-subtitle">请选择本机的运行模式，不同模式提供不同的功能</p>
    </div>

    <div class="mode-cards">
      <div
        v-for="mode in modes"
        :key="mode.key"
        class="mode-card"
        @click="selectMode(mode)"
      >
        <div class="mode-icon" :class="mode.iconClass">
          {{ mode.icon }}
        </div>
        <h2 class="mode-card-title">{{ mode.title }}</h2>
        <p class="mode-card-desc">{{ mode.desc }}</p>
        <ul class="mode-features">
          <li v-for="f in mode.features" :key="f">
            <span class="feature-dot"></span>
            {{ f }}
          </li>
        </ul>
        <div class="mode-card-action">
          进入 {{ mode.title }}
          <span class="arrow">&rarr;</span>
        </div>
      </div>
    </div>

    <div class="mode-footer">
      <p>选择后可在顶部标题栏点击模式名称切换</p>
    </div>
  </div>
</template>

<style scoped>
.mode-select {
  min-height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px 20px;
  background: var(--content-bg);
}

.mode-header {
  text-align: center;
  margin-bottom: 40px;
}

.mode-title {
  font-size: 26px;
  font-weight: 700;
  color: var(--text-primary);
  margin-bottom: 8px;
}

.mode-subtitle {
  font-size: 14px;
  color: var(--text-secondary);
}

.mode-cards {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 24px;
  max-width: 900px;
  width: 100%;
}

.mode-card {
  background: #fff;
  border-radius: 10px;
  box-shadow: var(--shadow);
  padding: 28px 24px;
  cursor: pointer;
  transition: all 0.2s ease;
  border: 2px solid transparent;
  display: flex;
  flex-direction: column;
}

.mode-card:hover {
  box-shadow: var(--shadow-md);
  border-color: var(--color-primary);
  transform: translateY(-2px);
}

.mode-icon {
  width: 56px;
  height: 56px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 28px;
  font-weight: 700;
  color: #fff;
  margin-bottom: 18px;
}

.icon-server {
  background: linear-gradient(135deg, #2563eb, #1d4ed8);
}

.icon-client {
  background: linear-gradient(135deg, #16a34a, #15803d);
}

.icon-operator {
  background: linear-gradient(135deg, #d97706, #c2410c);
}

.mode-card-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 8px;
}

.mode-card-desc {
  font-size: 13px;
  color: var(--text-secondary);
  line-height: 1.6;
  margin-bottom: 16px;
  flex: 1;
}

.mode-features {
  margin-bottom: 20px;
}

.mode-features li {
  font-size: 13px;
  color: var(--text-primary);
  padding: 4px 0;
  display: flex;
  align-items: center;
  gap: 8px;
}

.feature-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--color-primary);
  flex-shrink: 0;
}

.mode-card-action {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-primary);
  display: flex;
  align-items: center;
  gap: 6px;
  padding-top: 16px;
  border-top: 1px solid var(--border-color);
}

.mode-card-action .arrow {
  transition: transform 0.15s;
}

.mode-card:hover .mode-card-action .arrow {
  transform: translateX(4px);
}

.mode-footer {
  margin-top: 40px;
  text-align: center;
  font-size: 12px;
  color: var(--text-muted);
}
</style>
