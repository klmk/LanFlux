<script setup>
import { ref, computed, onMounted } from 'vue'
import { loadConfig, saveConfig, testConnectivity, getAppInfo } from '../tauri.js'

const loading = ref(true)
const saving = ref(false)
const testing = ref(false)
const serverAddr = ref('0.0.0.0:8443')
const appInfo = ref({ os: '', arch: '', version: '' })
const testResult = ref(null)
// null = 未检测, true = 运行中, false = 未运行
const serverRunning = ref(null)

/**
 * 解析服务端地址为 host 和 port
 * @param {string} addr 地址，如 0.0.0.0:8443
 */
function parseAddr(addr) {
  const cleaned = (addr || '').trim().replace(/^https?:\/\//, '').replace(/\/$/, '')
  const [host, portStr] = cleaned.split(':')
  return {
    host: host || '0.0.0.0',
    port: parseInt(portStr) || 8443
  }
}

/**
 * 计算 Web 管理后台访问地址
 * 监听 0.0.0.0 时用 localhost 访问
 */
const webAdminUrl = computed(() => {
  const { host, port } = parseAddr(serverAddr.value)
  const displayHost = host === '0.0.0.0' || host === '::' ? 'localhost' : host
  return `http://${displayHost}:${port}`
})

async function initConfig() {
  loading.value = true
  try {
    const config = await loadConfig()
    if (config.server_addr) {
      serverAddr.value = config.server_addr
    }
  } catch (e) {
    console.warn('加载配置失败，使用默认值', e)
  } finally {
    loading.value = false
  }
}

async function loadAppInfo() {
  try {
    appInfo.value = await getAppInfo()
  } catch {
    // 非 Tauri 环境，忽略
  }
}

async function handleSave() {
  if (!serverAddr.value.trim()) {
    alert('请输入监听地址')
    return
  }
  saving.value = true
  try {
    const config = await loadConfig()
    await saveConfig({
      ...config,
      mode: config.mode || 'server',
      server_addr: serverAddr.value.trim()
    })
  } catch (e) {
    console.error('保存配置失败', e)
    alert('保存配置失败: ' + (e.message || e))
  } finally {
    saving.value = false
  }
}

async function handleTest() {
  testing.value = true
  testResult.value = null
  serverRunning.value = null
  try {
    const { host, port } = parseAddr(serverAddr.value)
    const result = await testConnectivity(host, port)
    testResult.value = result
    serverRunning.value = !!result.success
  } catch (e) {
    testResult.value = {
      success: false,
      elapsed_ms: 0,
      message: '测试失败: ' + (e.message || e)
    }
    serverRunning.value = false
  } finally {
    testing.value = false
  }
}

function openWebAdmin() {
  const url = webAdminUrl.value
  try {
    if (window.__TAURI_INTERNALS__) {
      // 使用变量 + vite-ignore 避免构建时静态解析（插件仅在 Tauri 运行时可用）
      const moduleName = '@tauri-apps/plugin-shell'
      import(/* @vite-ignore */ moduleName)
        .then((shell) => shell.open(url))
        .catch(() => window.open(url, '_blank'))
    } else {
      window.open(url, '_blank')
    }
  } catch {
    window.open(url, '_blank')
  }
}

onMounted(() => {
  initConfig()
  loadAppInfo()
})
</script>

<template>
  <div>
    <!-- 提示信息 -->
    <div class="alert alert-warning">
      <span>桌面端服务端模式仅适用于开发测试与小规模场景，生产环境建议使用 Linux/Docker 部署。</span>
    </div>

    <!-- 服务端配置 -->
    <div class="card">
      <div class="card-title">
        <span>服务端配置</span>
      </div>
      <div class="form-group">
        <label class="form-label">监听地址</label>
        <div class="addr-row">
          <input
            class="form-input mono"
            v-model="serverAddr"
            placeholder="0.0.0.0:8443"
            :disabled="loading"
          />
          <button class="btn btn-primary" @click="handleSave" :disabled="saving || loading">
            {{ saving ? '保存中...' : '保存' }}
          </button>
        </div>
        <div class="form-hint">服务端监听地址，格式为 IP:端口，默认 0.0.0.0:8443</div>
      </div>
      <div class="server-actions">
        <button class="btn" @click="handleTest" :disabled="testing || loading">
          {{ testing ? '检测中...' : '检测服务状态' }}
        </button>
        <button class="btn btn-primary" @click="openWebAdmin">
          打开 Web 后台
        </button>
      </div>
    </div>

    <!-- 服务端运行状态 -->
    <div class="card">
      <div class="card-title">
        <span>服务端状态</span>
        <button class="btn btn-sm" @click="handleTest" :disabled="testing">刷新</button>
      </div>
      <div v-if="loading" class="loading-state">加载中...</div>
      <div v-else class="desc-list">
        <div class="desc-label">运行状态</div>
        <div class="desc-value">
          <span v-if="serverRunning === null" class="status-badge status-gray">未检测</span>
          <span v-else-if="serverRunning" class="status-badge status-green">运行中</span>
          <span v-else class="status-badge status-red">未运行</span>
        </div>
        <div class="desc-label">监听地址</div>
        <div class="desc-value mono">{{ serverAddr }}</div>
        <div class="desc-label">Web 管理后台</div>
        <div class="desc-value">
          <a href="javascript:void(0)" @click="openWebAdmin">{{ webAdminUrl }}</a>
        </div>
        <div class="desc-label">操作系统</div>
        <div class="desc-value">{{ appInfo.os || '-' }} / {{ appInfo.arch || '-' }}</div>
      </div>

      <!-- 连通性检测结果 -->
      <div v-if="testResult" class="test-result" :class="testResult.success ? 'test-success' : 'test-fail'">
        <span class="test-mark">{{ testResult.success ? '[OK]' : '[FAIL]' }}</span>
        <span class="test-msg">{{ testResult.message }}</span>
        <span v-if="testResult.elapsed_ms" class="test-time">({{ testResult.elapsed_ms }} ms)</span>
      </div>
    </div>

    <!-- 部署说明 -->
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

<style scoped>
.addr-row {
  display: flex;
  gap: 10px;
  align-items: stretch;
}

.addr-row .form-input {
  flex: 1;
}

.server-actions {
  display: flex;
  gap: 10px;
  margin-top: 4px;
}

.test-result {
  margin-top: 14px;
  padding: 10px 14px;
  border-radius: var(--radius);
  font-size: 13px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.test-success {
  background: var(--status-green-bg);
  color: var(--status-green);
}

.test-fail {
  background: var(--status-red-bg);
  color: var(--status-red);
}

.test-mark {
  font-weight: 700;
}

.test-time {
  color: var(--text-muted);
  font-size: 12px;
}
</style>
