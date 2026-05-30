<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { extractApiError } from '../api/client'
import { getSettings, updateSettings } from '../api/settings'

const loading = ref(false)
const saving = ref(false)
const errorMessage = ref('')
const successMessage = ref('')

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

    <article class="card stack management-card settings-card">
      <div>
        <div class="hint">节点延迟自动测试</div>
        <p class="card-copy">后台会用 Mihomo 内核进行真实代理链路测试，并保存最后延迟、状态和错误原因。</p>
      </div>

      <form class="form-grid settings-form" @submit.prevent="submit">
        <label class="checkbox-item settings-toggle">
          <input v-model="form.latency_auto_enabled" type="checkbox" />
          <span>
            <strong>启用自动测试延迟</strong>
            <div class="muted">关闭后仍可在节点管理页面手动测速。</div>
          </span>
        </label>

        <div>
          <label class="field-label" for="latency-core-path">Mihomo 内核路径</label>
          <input
            id="latency-core-path"
            v-model.trim="form.latency_core_path"
            class="input"
            placeholder="留空则自动查找 backend/mihomo，例如 mihomo\\mihomo.exe 或 mihomo/mihomo"
          />
          <div class="hint template-kind-hint">真实测速需要 Mihomo/Clash Meta 内核；留空时会优先查找后端目录下的 mihomo 文件夹，兼容 Windows exe 和 Linux 可执行文件。</div>
        </div>

        <div>
          <label class="field-label" for="latency-test-url">测试 URL</label>
          <input
            id="latency-test-url"
            v-model.trim="form.latency_test_url"
            class="input"
            placeholder="https://www.gstatic.com/generate_204"
          />
          <div class="hint template-kind-hint">Mihomo delay API 会让节点真实访问这个 URL，以得到完整代理链路延迟。</div>
        </div>

        <div>
          <label class="field-label" for="latency-interval">自动测速间隔</label>
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
          <div class="hint template-kind-hint">范围 5 到 1440 分钟；保存后下一轮后台循环生效。</div>
        </div>

        <div>
          <label class="field-label" for="latency-timeout">测速超时</label>
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
          <div class="hint template-kind-hint">范围 3 到 60 秒；超时会把节点标记为不可达。</div>
        </div>

        <div class="compat-panel">
          <span class="status-badge status-badge-neutral">真实链路</span>
          <p class="compat-copy">
            这里不是 TCPING。系统会临时启动 Mihomo，加载单个节点，调用内核延迟测试接口访问测试 URL，因此会验证协议握手、认证、TLS/Reality/HY2 等真实配置。
          </p>
        </div>

        <div class="inline-actions">
          <button class="button button-accent" type="submit" :disabled="saving">
            {{ saving ? '保存中...' : '保存设置' }}
          </button>
        </div>
      </form>
    </article>
  </section>
</template>
