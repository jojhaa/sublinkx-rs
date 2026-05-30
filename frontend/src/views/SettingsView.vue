<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { extractApiError } from '../api/client'
import {
  downloadMihomoCore,
  getMihomoCoreStatus,
  getSettings,
  updateSettings,
  type MihomoCoreStatus,
} from '../api/settings'

const loading = ref(false)
const saving = ref(false)
const checkingCore = ref(false)
const downloadingCore = ref(false)
const errorMessage = ref('')
const successMessage = ref('')
const mihomoCore = ref<MihomoCoreStatus | null>(null)

const form = reactive({
  latency_auto_enabled: true,
  latency_interval_minutes: 30,
  latency_core_path: '',
  latency_test_url: 'https://www.gstatic.com/generate_204',
  latency_timeout_secs: 10,
})

async function load() {
  loading.value = true
  errorMessage.value = ''
  try {
    const response = await getSettings()
    form.latency_auto_enabled = response.data.latency_auto_enabled
    form.latency_interval_minutes = response.data.latency_interval_minutes
    form.latency_core_path = response.data.latency_core_path
    form.latency_test_url = response.data.latency_test_url
    form.latency_timeout_secs = response.data.latency_timeout_secs
    await checkMihomoCore(false)
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    loading.value = false
  }
}

async function submit() {
  saving.value = true
  errorMessage.value = ''
  successMessage.value = ''
  try {
    const response = await updateSettings({
      latency_auto_enabled: form.latency_auto_enabled,
      latency_interval_minutes: form.latency_interval_minutes,
      latency_core_path: form.latency_core_path,
      latency_test_url: form.latency_test_url,
      latency_timeout_secs: form.latency_timeout_secs,
    })
    form.latency_auto_enabled = response.data.latency_auto_enabled
    form.latency_interval_minutes = response.data.latency_interval_minutes
    form.latency_core_path = response.data.latency_core_path
    form.latency_test_url = response.data.latency_test_url
    form.latency_timeout_secs = response.data.latency_timeout_secs
    successMessage.value = '设置已保存。'
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    saving.value = false
  }
}

async function checkMihomoCore(showMessage = true) {
  checkingCore.value = true
  errorMessage.value = ''
  if (showMessage) {
    successMessage.value = ''
  }
  try {
    const response = await getMihomoCoreStatus()
    mihomoCore.value = response.data
    if (showMessage) {
      successMessage.value = response.data.installed ? 'Mihomo 内核检测完成。' : '未检测到 Mihomo 内核，可以点击下载。'
    }
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    checkingCore.value = false
  }
}

async function installMihomoCore() {
  downloadingCore.value = true
  errorMessage.value = ''
  successMessage.value = ''
  try {
    const response = await downloadMihomoCore()
    form.latency_core_path = response.data.path
    successMessage.value = `已下载 ${response.data.version}：${response.data.asset_name}`
    await checkMihomoCore(false)
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    downloadingCore.value = false
  }
}

onMounted(load)
</script>

<template>
  <section class="stack">
    <header class="page-header">
      <div>
        <span class="eyebrow">Settings</span>
        <h2 class="page-title">系统设置</h2>
        <p class="page-copy">集中管理后台自动任务和运行策略。</p>
      </div>
      <button class="button button-ghost" type="button" :disabled="loading" @click="load">
        {{ loading ? '刷新中...' : '刷新' }}
      </button>
    </header>

    <div v-if="errorMessage" class="error-banner">{{ errorMessage }}</div>
    <div v-if="successMessage" class="success-banner">{{ successMessage }}</div>

    <article class="settings-console">
      <div class="settings-console-header">
        <div>
          <div class="hint">Latency Control Plane</div>
          <h3>Mihomo 内核与延迟测试</h3>
          <p class="card-copy">检测内核、控制自动测速，并保存节点最后延迟与错误原因。</p>
        </div>
        <span class="settings-console-mark">MIHOMO</span>
      </div>

      <form class="settings-console-grid" @submit.prevent="submit">
        <section class="settings-console-panel settings-core-panel">
          <div class="settings-panel-title">
            <span class="settings-panel-index">01</span>
            <div>
              <strong>Core Readiness</strong>
              <div class="hint">检测、下载与运行路径</div>
            </div>
          </div>

          <div class="core-meter">
            <div>
              <span class="status-badge" :class="mihomoCore?.installed ? 'status-badge-ok' : 'status-badge-warn'">
                {{ mihomoCore?.installed ? 'READY' : 'MISSING' }}
              </span>
              <strong>{{ mihomoCore?.installed ? '内核可用' : '需要下载内核' }}</strong>
            </div>
            <span class="metric-chip">{{ mihomoCore ? `${mihomoCore.os}/${mihomoCore.arch}` : 'checking' }}</span>
          </div>

          <div class="settings-core-grid">
            <span class="hint">路径</span>
            <code class="token-link">{{ mihomoCore?.path || 'backend/mihomo/' }}</code>
            <span class="hint">版本</span>
            <span>{{ mihomoCore?.version || '未知' }}</span>
          </div>

          <div>
            <label class="field-label" for="latency-core-path">自定义内核路径</label>
            <input
              id="latency-core-path"
              v-model.trim="form.latency_core_path"
              class="input"
              placeholder="留空则自动查找 backend/mihomo"
            />
          </div>

          <div class="settings-button-rail">
            <button class="button button-ghost button-compact" type="button" :disabled="checkingCore" @click="checkMihomoCore()">
              {{ checkingCore ? '检测中...' : '检测内核' }}
            </button>
            <button
              class="button button-accent button-compact"
              type="button"
              :disabled="downloadingCore || mihomoCore?.supported === false"
              @click="installMihomoCore"
            >
              {{ downloadingCore ? '下载中...' : mihomoCore?.installed ? '下载/更新内核' : '下载内核' }}
            </button>
          </div>

          <p class="compat-copy">
            自动下载遵循官方 FAQ：AMD64 默认选择 v1 构建，Linux 旧内核优先 go123，并保存到后端 mihomo 文件夹。
          </p>
        </section>

        <section class="settings-console-panel">
          <div class="settings-panel-title">
            <span class="settings-panel-index">02</span>
            <div>
              <strong>Scheduler</strong>
              <div class="hint">后台自动测速节奏</div>
            </div>
          </div>

          <label class="settings-switch">
            <input v-model="form.latency_auto_enabled" type="checkbox" />
            <span></span>
            <div>
              <strong>启用自动延迟测试</strong>
              <small>关闭后仍可在节点管理页面手动测速。</small>
            </div>
          </label>

          <div class="settings-control-grid">
            <label>
              <span class="field-label" for="latency-interval">自动测速间隔</span>
              <div class="settings-inline-field">
                <input
                  id="latency-interval"
                  v-model.number="form.latency_interval_minutes"
                  class="input"
                  max="1440"
                  min="5"
                  type="number"
                />
                <span class="metric-chip">分钟</span>
              </div>
            </label>

            <label>
              <span class="field-label" for="latency-timeout">测速超时</span>
              <div class="settings-inline-field">
                <input
                  id="latency-timeout"
                  v-model.number="form.latency_timeout_secs"
                  class="input"
                  max="60"
                  min="3"
                  type="number"
                />
                <span class="metric-chip">秒</span>
              </div>
            </label>
          </div>

          <div>
            <label class="field-label" for="latency-test-url">测试 URL</label>
            <input
              id="latency-test-url"
              v-model.trim="form.latency_test_url"
              class="input"
              placeholder="https://www.gstatic.com/generate_204"
            />
            <div class="hint template-kind-hint">Mihomo delay API 会让节点真实访问这个 URL。</div>
          </div>
        </section>

        <section class="settings-console-panel settings-proof-panel">
          <span class="status-badge status-badge-neutral">真实链路</span>
          <p class="compat-copy">
            这里不是 TCPING。系统会临时启动 Mihomo，加载单个节点，调用内核延迟测试接口访问测试 URL，因此会验证协议握手、认证、TLS/Reality/HY2 等真实配置。
          </p>
        </section>

        <div class="settings-save-rail">
          <button class="button button-accent" type="submit" :disabled="saving">
            {{ saving ? '保存中...' : '保存设置' }}
          </button>
        </div>
      </form>
    </article>
  </section>
</template>
