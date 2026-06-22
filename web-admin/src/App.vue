<script setup>
import { computed } from 'vue'
import { useRoute } from 'vue-router'

const route = useRoute()

const navItems = [
  { path: '/', title: '仪表盘', icon: '▣' },
  { path: '/nodes', title: '节点管理', icon: '◉' },
  { path: '/segments', title: '网段管理', icon: '▦' },
  { path: '/mappings', title: '映射关系', icon: '⇄' },
  { path: '/policies', title: '权限配置', icon: '⚙' },
  { path: '/pools', title: '地址池配置', icon: '◈' },
  { path: '/sessions', title: '连接会话', icon: '⇆' },
  { path: '/logs', title: '日志诊断', icon: '☰' }
]

const currentTitle = computed(() => route.meta.title || '组网工具管理后台')

// 判断导航项是否激活（处理子路由高亮）
function isActive(path) {
  if (path === '/') {
    return route.path === '/'
  }
  return route.path === path || route.path.startsWith(path + '/')
}
</script>

<template>
  <div class="layout">
    <!-- 侧边导航栏 -->
    <aside class="sidebar">
      <div class="sidebar-header">
        <div class="logo-title">组网工具</div>
        <div class="logo-sub">服务端管理后台</div>
      </div>
      <nav class="nav">
        <router-link
          v-for="item in navItems"
          :key="item.path"
          :to="item.path"
          class="nav-item"
          :class="{ active: isActive(item.path) }"
        >
          <span class="nav-icon">{{ item.icon }}</span>
          <span class="nav-text">{{ item.title }}</span>
        </router-link>
      </nav>
      <div class="sidebar-footer">
        <span class="version-tag">v0.1.0</span>
      </div>
    </aside>

    <!-- 主区域 -->
    <div class="main">
      <header class="topbar">
        <div class="topbar-title">{{ currentTitle }}</div>
        <div class="topbar-right">
          <span class="topbar-brand">组网工具</span>
          <span class="topbar-version">v0.1.0</span>
        </div>
      </header>
      <main class="content">
        <router-view />
      </main>
    </div>
  </div>
</template>

<style scoped>
.layout {
  display: flex;
  height: 100%;
}

/* 侧边栏 */
.sidebar {
  width: var(--sidebar-width);
  background: var(--sidebar-bg);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
}

.sidebar-header {
  padding: 20px 18px 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.logo-title {
  font-size: 18px;
  font-weight: 700;
  color: #fff;
  letter-spacing: 1px;
}

.logo-sub {
  font-size: 12px;
  color: var(--sidebar-text);
  margin-top: 4px;
}

.nav {
  flex: 1;
  padding: 10px 0;
  overflow-y: auto;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 20px;
  color: var(--sidebar-text);
  font-size: 14px;
  transition: all 0.15s ease;
  border-left: 3px solid transparent;
  text-decoration: none;
}

.nav-item:hover {
  background: var(--sidebar-bg-hover);
  color: var(--sidebar-text-active);
  text-decoration: none;
}

.nav-item.active {
  background: var(--sidebar-active-bg);
  color: var(--sidebar-text-active);
  border-left-color: #60a5fa;
}

.nav-icon {
  font-size: 15px;
  width: 18px;
  text-align: center;
  flex-shrink: 0;
}

.sidebar-footer {
  padding: 14px 20px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
}

.version-tag {
  font-size: 12px;
  color: var(--sidebar-text);
}

/* 主区域 */
.main {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.topbar {
  height: 54px;
  background: #fff;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 22px;
  flex-shrink: 0;
}

.topbar-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
}

.topbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
}

.topbar-brand {
  color: var(--text-secondary);
  font-weight: 500;
}

.topbar-version {
  color: #fff;
  background: var(--color-primary);
  padding: 2px 8px;
  border-radius: 10px;
  font-size: 12px;
}

.content {
  flex: 1;
  overflow-y: auto;
  padding: 18px 22px;
}
</style>
