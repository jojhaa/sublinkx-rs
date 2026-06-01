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
import { useI18n } from '../i18n'

const { t } = useI18n()
const loading = ref(false)
const saving = ref(false)
const checkingCore = ref(false)
const downloadingCore = ref(false)
const errorMessage = ref('')
const successMessage = ref('')
const mihomoCore = ref<MihomoCoreStatus | null>(null)

const form = reactive({
  public_base_url: '',
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
    form.public_base_url = response.data.public_base_url
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
      public_base_url: form.public_base_url,
      latency_auto_enabled: form.latency_auto_enabled,
      latency_interval_minutes: form.latency_interval_minutes,
      latency_core_path: form.latency_core_path,
      latency_test_url: form.latency_test_url,
      latency_timeout_secs: form.latency_timeout_secs,
    })
    form.public_base_url = response.data.public_base_url
    form.latency_auto_enabled = response.data.latency_auto_enabled
    form.latency_interval_minutes = response.data.latency_interval_minutes
    form.latency_core_path = response.data.latency_core_path
    form.latency_test_url = response.data.latency_test_url
    form.latency_timeout_secs = response.data.latency_timeout_secs
    successMessage.value = t('settingsSaved')
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
      successMessage.value = response.data.installed ? t('coreCheckDone') : t('coreNotFound')
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
    successMessage.value = t('coreDownloaded', { version: response.data.version, asset: response.data.asset_name })
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
        <span class="eyebrow">{{ t('settingsEyebrow') }}</span>
        <h2 class="page-title">{{ t('settingsTitle') }}</h2>
        <p class="page-copy">{{ t('settingsCopy') }}</p>
      </div>
      <button class="button button-ghost" type="button" :disabled="loading" @click="load">
        {{ loading ? t('refreshing') : t('refresh') }}
      </button>
    </header>

    <div v-if="errorMessage" class="error-banner">{{ errorMessage }}</div>
    <div v-if="successMessage" class="success-banner">{{ successMessage }}</div>

    <article class="settings-console">
      <div class="settings-console-header">
        <div>
          <div class="hint">{{ t('latencyConsoleHint') }}</div>
          <h3>{{ t('latencyConsoleTitle') }}</h3>
          <p class="card-copy">{{ t('latencyConsoleCopy') }}</p>
        </div>
        <span class="settings-console-mark">MIHOMO</span>
      </div>

      <form class="settings-console-grid" @submit.prevent="submit">
        <section class="settings-console-panel settings-core-panel">
          <div class="settings-panel-title">
            <span class="settings-panel-index">01</span>
            <div>
              <strong>{{ t('coreReadiness') }}</strong>
              <div class="hint">{{ t('coreReadinessHint') }}</div>
            </div>
          </div>

          <div class="core-meter">
            <div>
              <span class="status-badge" :class="mihomoCore?.installed ? 'status-badge-ok' : 'status-badge-warn'">
                {{ mihomoCore?.installed ? 'READY' : 'MISSING' }}
              </span>
              <strong>{{ mihomoCore?.installed ? t('coreReady') : t('coreMissing') }}</strong>
            </div>
            <span class="metric-chip">{{ mihomoCore ? `${mihomoCore.os}/${mihomoCore.arch}` : 'checking' }}</span>
          </div>

          <div class="settings-core-grid">
            <span class="hint">{{ t('path') }}</span>
            <code class="token-link">{{ mihomoCore?.path || 'backend/mihomo/' }}</code>
            <span class="hint">{{ t('version') }}</span>
            <span>{{ mihomoCore?.version || t('unknown') }}</span>
          </div>

          <div>
            <label class="field-label" for="latency-core-path">{{ t('customCorePath') }}</label>
            <input
              id="latency-core-path"
              v-model.trim="form.latency_core_path"
              class="input"
              :placeholder="t('customCorePlaceholder')"
            />
          </div>

          <div class="settings-button-rail">
            <button class="button button-ghost button-compact" type="button" :disabled="checkingCore" @click="checkMihomoCore()">
              {{ checkingCore ? t('checkingCore') : t('checkCore') }}
            </button>
            <button
              class="button button-accent button-compact"
              type="button"
              :disabled="downloadingCore || mihomoCore?.supported === false"
              @click="installMihomoCore"
            >
              {{ downloadingCore ? t('downloading') : mihomoCore?.installed ? t('updateCore') : t('downloadCore') }}
            </button>
          </div>

          <p class="compat-copy">
            {{ t('coreFaqNote') }}
          </p>
        </section>

        <section class="settings-console-panel">
          <div class="settings-panel-title">
            <span class="settings-panel-index">02</span>
            <div>
              <strong>{{ t('siteAccess') }}</strong>
              <div class="hint">{{ t('siteAccessHint') }}</div>
            </div>
          </div>

          <div>
            <label class="field-label" for="public-base-url">{{ t('publicBaseUrl') }}</label>
            <input
              id="public-base-url"
              v-model.trim="form.public_base_url"
              class="input"
              placeholder="https://example.com"
            />
            <div class="hint template-kind-hint">{{ t('publicBaseUrlHint') }}</div>
          </div>
        </section>

        <section class="settings-console-panel">
          <div class="settings-panel-title">
            <span class="settings-panel-index">03</span>
            <div>
              <strong>{{ t('scheduler') }}</strong>
              <div class="hint">{{ t('schedulerHint') }}</div>
            </div>
          </div>

          <label class="settings-switch">
            <input v-model="form.latency_auto_enabled" type="checkbox" />
            <span></span>
            <div>
              <strong>{{ t('enableAutoLatency') }}</strong>
              <small>{{ t('enableAutoLatencyHint') }}</small>
            </div>
          </label>

          <div class="settings-control-grid">
            <label>
              <span class="field-label" for="latency-interval">{{ t('latencyInterval') }}</span>
              <div class="settings-inline-field">
                <input
                  id="latency-interval"
                  v-model.number="form.latency_interval_minutes"
                  class="input"
                  max="1440"
                  min="5"
                  type="number"
                />
                <span class="metric-chip">{{ t('minute') }}</span>
              </div>
            </label>

            <label>
              <span class="field-label" for="latency-timeout">{{ t('latencyTimeout') }}</span>
              <div class="settings-inline-field">
                <input
                  id="latency-timeout"
                  v-model.number="form.latency_timeout_secs"
                  class="input"
                  max="60"
                  min="3"
                  type="number"
                />
                <span class="metric-chip">{{ t('second') }}</span>
              </div>
            </label>
          </div>

          <div>
            <label class="field-label" for="latency-test-url">{{ t('testUrl') }}</label>
            <input
              id="latency-test-url"
              v-model.trim="form.latency_test_url"
              class="input"
              placeholder="https://www.gstatic.com/generate_204"
            />
            <div class="hint template-kind-hint">{{ t('testUrlHint') }}</div>
          </div>
        </section>

        <section class="settings-console-panel settings-proof-panel">
          <span class="status-badge status-badge-neutral">{{ t('realLink') }}</span>
          <p class="compat-copy">
            {{ t('realLinkCopy') }}
          </p>
        </section>

        <div class="settings-save-rail">
          <button class="button button-accent" type="submit" :disabled="saving">
            {{ saving ? t('saving') : t('saveSettings') }}
          </button>
        </div>
      </form>
    </article>
  </section>
</template>
