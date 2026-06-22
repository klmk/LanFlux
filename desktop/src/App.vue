<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { getAppInfo, loadConfig, saveConfig } from './tauri.js'

const route = useRoute()
const router = useRouter()

const appVersion = ref('0.1.0')
const currentMode = ref('')

// 各模式侧边导航
const navConfig = {
  client: [
    { path: '/client', title: '主界面', icon: '▣' },
    { path: '/client/segments', title: '网段管理', icon: '▦' },
    { path: '/client/status', title: '连接状态', icon: '◉' }
  ],
  operator: [
    { path: '/operator', title: '主界面', icon: '▣' },
    { path: '/operator/segments', title: '可访问网段', icon: '▦' },
    { path: '/operator/test', title: '连通性测试', icon: '⇄' }
  ],
  server: [
    { path: '/server', title: '服务端状态', icon: '◉' }
  ]
}

const navItems = computed(() => navConfig[currentMode.value] || [])

const currentTitle = computed(() => route.meta.title || 'NetTool 组网工具')

const isModeSelect = computed(() => route.path === '/')

function isActive(path) {
  if (path === '/client' || path === '/operator' || path === '/server') {
    return route.path === path
  }
  return route.path === path || route.path.startsWith(path + '/')
}

function switchMode() {
  currentMode.value = ''
  router.push('/')
}

async function initApp() {
  try {
    const info = await getAppInfo()
    appVersion.value = info.version
  } catch {
    // 非 Tauri 环境（浏览器调试），使用默认版本
  }

  try {
    const config = await loadConfig()
    if (config.mode) {
      currentMode.value = config.mode
    }
  } catch {
    // 配置加载失败，停留在模式选择
  }
}

onMounted(initApp)
</script>

<template>
  <div class="app-layout">
    <!-- 顶部标题栏 -->
    <header class="titlebar">
      <div class="titlebar-left">
        <span class="titlebar-logo">NetTool</span>
        <span class="titlebar-name">组网工具</span>
      </div>
      <div class="titlebar-center">
        <span v-if="currentMode" class="titlebar-mode" @click="switchMode">
          {{ currentMode === 'server' ? '服务端' : currentMode === 'client' ? '客户端' : '实施端' }}
        </span>
      </div>
      <div class="titlebar-right">
        <span class="titlebar-version">v{{ appVersion }}</span>
      </div>
    </header>

    <!-- 模式选择页面（无侧边栏） -->
    <main v-if="isModeSelect" class="content-full">
      <router-view />
    </main>

    <!-- 带侧边栏的布局 -->
    <div v-else class="body-layout">
      <aside class="sidebar">
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
          <button class="btn btn-sm" style="width: 100%" @click="switchMode">切换模式</button>
        </div>
      </aside>
      <main class="content">
        <div class="content-header">{{ currentTitle }}</div>
        <div class="content-body">
          <router-view />
        </div>
      </main>
    </div>
  </div>
</template>

<style scoped>
.app-layout {
  display: flex;
  flex-direction: column;
  height: 100%;
}

/* 顶部标题栏 */
.titlebar {
  height: var(--titlebar-height);
  background: var(--sidebar-bg);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 16px;
  flex-shrink: 0;
}

.titlebar-left {
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.titlebar-logo {
  font-size: 16px;
  font-weight: 700;
  color: #fff;
  letter-spacing: 0.5px;
}

.titlebar-name {
  font-size: 13px;
  color: var(--sidebar-text);
}

.titlebar-center {
  flex: 1;
  text-align: center;
}

.titlebar-mode {
  font-size: 13px;
  color: var(--sidebar-text);
  cursor: pointer;
  padding: 2px 12px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.08);
  transition: all 0.15s;
}

.titlebar-mode:hover {
  background: rgba(255, 255, 255, 0.15);
  color: #fff;
}

.titlebar-right {
  display: flex;
  align-items: center;
}

.titlebar-version {
  font-size: 12px;
  color: #fff;
  background: var(--color-primary);
  padding: 2px 8px;
  border-radius: 10px;
}

/* 全屏内容（模式选择） */
.content-full {
  flex: 1;
  overflow-y: auto;
}

/* 带侧边栏布局 */
.body-layout {
  flex: 1;
  display: flex;
  overflow: hidden;
}

.sidebar {
  width: var(--sidebar-width);
  background: var(--sidebar-bg);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
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
  padding: 12px 16px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
}

/* 主内容区 */
.content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.content-header {
  height: 44px;
  background: #fff;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  padding: 0 22px;
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
  flex-shrink: 0;
}

.content-body {
  flex: 1;
  overflow-y: auto;
  padding: 18px 22px;
}
</style>
