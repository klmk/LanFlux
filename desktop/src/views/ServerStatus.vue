<script setup>
import { ref, onMounted } from 'vue'
import { getDashboard } from '../api.js'
import { getAppInfo } from '../tauri.js'

const loading = ref(true)
const stats = ref({
  online_node_count: 0,
  registered_segment_count: 0,
  client_count: 0,
  operator_count: 0
})
const appInfo = ref({ os: '', arch: '' })
const webAdminUrl = ref('http://localhost:8443')

async function loadData() {
  loading.value = true
  try {
    const data = await getDashboard()
    stats.value = data
  } catch (e) {
    console.error('加载服务端状态失败', e)
  } finally {
    loading.value = false
  }
}

async function loadAppInfo() {
  try {
    appInfo.value = await getAppInfo()
  } catch {
    // ignore
  }
}

function openWebAdmin() {
  // 在 Tauri 中打开外部浏览器（如安装了 shell 插件则使用，否则回退到 window.open）
  try {
    if (window.__TAURI_INTERNALS__) {
      import('@tauri-apps/plugin-shell')
        .then((shell) => shell.open(webAdminUrl.value))
        .catch(() => window.open(webAdminUrl.value, '_blank'))
    } else {
      window.open(webAdminUrl.value, '_blank')
    }
  } catch {
    window.open(webAdminUrl.value, '_blank')
  }
}

onMounted(() => {
  loadData()
  loadAppInfo()
})
</script>

<template>
  <div>
    <!-- 提示信息 -->
    <div class="alert alert-warning">
      <span>生产环境建议使用 Linux/Docker 部署服务端，桌面端服务端模式仅适用于开发测试与小规模场景。</span>
    </div>

    <!-- 服务端运行状态 -->
    <div class="card">
      <div class="card-title">
        <span>服务端运行状态</span>
        <button class="btn btn-sm" @click="loadData">刷新</button>
      </div>
      <div v-if="loading" class="loading-state">加载中...</div>
      <div v-else class="desc-list">
        <div class="desc-label">运行状态</div>
        <div class="desc-value">
          <span class="status-badge status-green">运行中</span>
        </div>
        <div class="desc-label">监听地址</div>
        <div class="desc-value mono">0.0.0.0:8443</div>
        <div class="desc-label">Web 管理后台</div>
        <div class="desc-value">
          <a href="javascript:void(0)" @click="openWebAdmin">{{ webAdminUrl }}</a>
          <button class="btn btn-sm" style="margin-left: 8px" @click="openWebAdmin">打开</button>
        </div>
        <div class="desc-label">操作系统</div>
        <div class="desc-value">{{ appInfo.os }} / {{ appInfo.arch }}</div>
      </div>
    </div>

    <!-- 基本统计 -->
    <div class="stat-grid">
      <div class="stat-card">
        <div class="stat-icon blue">◉</div>
        <div>
          <div class="stat-value">{{ stats.online_node_count }}</div>
          <div class="stat-label">在线节点</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-icon green">▦</div>
        <div>
          <div class="stat-value">{{ stats.registered_segment_count }}</div>
          <div class="stat-label">已登记网段</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-icon cyan">◈</div>
        <div>
          <div class="stat-value">{{ stats.client_count }}</div>
          <div class="stat-label">客户端数</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-icon red">!</div>
        <div>
          <div class="stat-value">{{ stats.operator_count }}</div>
          <div class="stat-label">实施端数</div>
        </div>
      </div>
    </div>

    <!-- 说明 -->
    <div class="card">
      <div class="card-title">服务端说明</div>
      <div class="alert alert-info">
        <div>
          <p style="margin-bottom: 8px"><strong>服务端职责：</strong></p>
          <ul style="padding-left: 16px; line-height: 1.8">
            <li>管理所有客户端与实施端节点的注册、心跳与状态</li>
            <li>为上报的网段分配映射网段地址，维护映射关系</li>
            <li>根据权限策略向节点下发路由</li>
            <li>提供 Web 管理后台供管理员操作</li>
          </ul>
        </div>
      </div>
      <div class="alert alert-warning" style="margin-top: 12px">
        <div>
          <strong>部署建议：</strong>生产环境请使用 Linux 服务器或 Docker 部署，
          参考项目根目录下的 <span class="mono">docker-compose.yml</span> 与 <span class="mono">Dockerfile</span>。
        </div>
      </div>
    </div>
  </div>
</template>
