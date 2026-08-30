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
import { getNodeIpIntelligenceStatus, type NodeIpIntelligenceStatus } from '../api/nodeIpProbes'
import { useI18n } from '../i18n'

const { t } = useI18n()
const loading = ref(false)
const saving = ref(false)
const checkingCore = ref(false)
const downloadingCore = ref(false)
const errorMessage = ref('')
const successMessage = ref('')
const mihomoCore = ref<MihomoCoreStatus | null>(null)
const intelligenceStatus = ref<NodeIpIntelligenceStatus | null>(null)

const form = reactive({
  public_base_url: '',
  latency_auto_enabled: true,
  latency_interval_minutes: 30,
  latency_concurrency: 2,
  latency_core_path: '',
  latency_test_url: 'https://cp.cloudflare.com/generate_204',
  latency_timeout_secs: 10,
  ip_probe_auto_enabled: false,
  ip_probe_interval_minutes: 360,
  ip_probe_after_upstream_import: true,
  country_detection_auto_enabled: true,
  country_detection_interval_minutes: 5,
  connectivity_default_target: 'system_default' as 'system_default' | 'cloudflare_204' | 'google_204',
  connectivity_default_rounds: 3 as 1 | 3 | 5,
  connectivity_sync_last_latency: true,
  public_export_cache_ttl_seconds: 30,
  public_export_ip_limit_per_minute: 120,
  public_export_global_limit_per_minute: 1200,
  mihomo_country_load_min_nodes: 2,
  mihomo_country_fallback_min_nodes: 3,
})

async function load() {
  loading.value = true
  errorMessage.value = ''
  try {
    const response = await getSettings()
    form.public_base_url = response.data.public_base_url
    form.latency_auto_enabled = response.data.latency_auto_enabled
    form.latency_interval_minutes = response.data.latency_interval_minutes
    form.latency_concurrency = response.data.latency_concurrency
    form.latency_core_path = response.data.latency_core_path
    form.latency_test_url = response.data.latency_test_url
    form.latency_timeout_secs = response.data.latency_timeout_secs
    form.ip_probe_auto_enabled = response.data.ip_probe_auto_enabled
    form.ip_probe_interval_minutes = response.data.ip_probe_interval_minutes
    form.ip_probe_after_upstream_import = response.data.ip_probe_after_upstream_import
    form.country_detection_auto_enabled = response.data.country_detection_auto_enabled
    form.country_detection_interval_minutes = response.data.country_detection_interval_minutes
    form.connectivity_default_target = response.data.connectivity_default_target
    form.connectivity_default_rounds = response.data.connectivity_default_rounds
    form.connectivity_sync_last_latency = response.data.connectivity_sync_last_latency
    form.public_export_cache_ttl_seconds = response.data.public_export_cache_ttl_seconds
    form.public_export_ip_limit_per_minute = response.data.public_export_ip_limit_per_minute
    form.public_export_global_limit_per_minute = response.data.public_export_global_limit_per_minute
    form.mihomo_country_load_min_nodes = response.data.mihomo_country_load_min_nodes
    form.mihomo_country_fallback_min_nodes = response.data.mihomo_country_fallback_min_nodes
    await Promise.all([checkMihomoCore(false), loadIntelligenceStatus()])
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
      latency_concurrency: form.latency_concurrency,
      latency_core_path: form.latency_core_path,
      latency_test_url: form.latency_test_url,
      latency_timeout_secs: form.latency_timeout_secs,
      ip_probe_auto_enabled: form.ip_probe_auto_enabled,
      ip_probe_interval_minutes: form.ip_probe_interval_minutes,
      ip_probe_after_upstream_import: form.ip_probe_after_upstream_import,
      country_detection_auto_enabled: form.country_detection_auto_enabled,
      country_detection_interval_minutes: form.country_detection_interval_minutes,
      connectivity_default_target: form.connectivity_default_target,
      connectivity_default_rounds: form.connectivity_default_rounds,
      connectivity_sync_last_latency: form.connectivity_sync_last_latency,
      public_export_cache_ttl_seconds: form.public_export_cache_ttl_seconds,
      public_export_ip_limit_per_minute: form.public_export_ip_limit_per_minute,
      public_export_global_limit_per_minute: form.public_export_global_limit_per_minute,
      mihomo_country_load_min_nodes: form.mihomo_country_load_min_nodes,
      mihomo_country_fallback_min_nodes: form.mihomo_country_fallback_min_nodes,
    })
    form.public_base_url = response.data.public_base_url
    form.latency_auto_enabled = response.data.latency_auto_enabled
    form.latency_interval_minutes = response.data.latency_interval_minutes
    form.latency_concurrency = response.data.latency_concurrency
    form.latency_core_path = response.data.latency_core_path
    form.latency_test_url = response.data.latency_test_url
    form.latency_timeout_secs = response.data.latency_timeout_secs
    form.ip_probe_auto_enabled = response.data.ip_probe_auto_enabled
    form.ip_probe_interval_minutes = response.data.ip_probe_interval_minutes
    form.ip_probe_after_upstream_import = response.data.ip_probe_after_upstream_import
    form.country_detection_auto_enabled = response.data.country_detection_auto_enabled
    form.country_detection_interval_minutes = response.data.country_detection_interval_minutes
    form.connectivity_default_target = response.data.connectivity_default_target
    form.connectivity_default_rounds = response.data.connectivity_default_rounds
    form.connectivity_sync_last_latency = response.data.connectivity_sync_last_latency
    form.public_export_cache_ttl_seconds = response.data.public_export_cache_ttl_seconds
    form.public_export_ip_limit_per_minute = response.data.public_export_ip_limit_per_minute
    form.public_export_global_limit_per_minute = response.data.public_export_global_limit_per_minute
    form.mihomo_country_load_min_nodes = response.data.mihomo_country_load_min_nodes
    form.mihomo_country_fallback_min_nodes = response.data.mihomo_country_fallback_min_nodes
    successMessage.value = t('settingsSaved')
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    saving.value = false
  }
}

async function loadIntelligenceStatus() {
  try {
    intelligenceStatus.value = await getNodeIpIntelligenceStatus()
  } catch {
    intelligenceStatus.value = null
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

            <label>
              <span class="field-label" for="latency-concurrency">{{ t('latencyConcurrency') }}</span>
              <div class="settings-inline-field">
                <input
                  id="latency-concurrency"
                  v-model.number="form.latency_concurrency"
                  class="input"
                  max="8"
                  min="1"
                  type="number"
                />
                <span class="metric-chip">{{ t('concurrent') }}</span>
              </div>
            </label>
          </div>

          <div>
            <label class="field-label" for="latency-test-url">{{ t('testUrl') }}</label>
            <input
              id="latency-test-url"
              v-model.trim="form.latency_test_url"
              class="input"
              placeholder="https://cp.cloudflare.com/generate_204"
            />
            <div class="hint template-kind-hint">{{ t('testUrlHint') }}</div>
          </div>

          <div class="settings-panel-divider"></div>

          <label class="settings-switch">
            <input v-model="form.ip_probe_auto_enabled" type="checkbox" />
            <span></span>
            <div>
              <strong>{{ t('enableAutoIpProbe') }}</strong>
              <small>{{ t('enableAutoIpProbeHint') }}</small>
            </div>
          </label>

          <label>
            <span class="field-label" for="ip-probe-interval">{{ t('ipProbeInterval') }}</span>
            <div class="settings-inline-field">
              <input
                id="ip-probe-interval"
                v-model.number="form.ip_probe_interval_minutes"
                class="input"
                max="10080"
                min="15"
                type="number"
              />
              <span class="metric-chip">{{ t('minute') }}</span>
            </div>
          </label>

          <label class="settings-switch">
            <input v-model="form.ip_probe_after_upstream_import" type="checkbox" />
            <span></span>
            <div>
              <strong>{{ t('probeAfterUpstreamImport') }}</strong>
              <small>{{ t('probeAfterUpstreamImportHint') }}</small>
            </div>
          </label>
        </section>

        <section class="settings-console-panel">
          <div class="settings-panel-title">
            <span class="settings-panel-index">04</span>
            <div>
              <strong>{{ t('countryDetection') }}</strong>
              <div class="hint">{{ t('countryDetectionHint') }}</div>
            </div>
          </div>

          <div class="core-meter">
            <div>
              <span class="status-badge" :class="intelligenceStatus?.enabled ? 'status-badge-ok' : 'status-badge-warn'">
                {{ intelligenceStatus?.enabled ? 'ON' : 'OFF' }}
              </span>
              <strong>{{ intelligenceStatus?.enabled ? t('countryServiceReady') : t('countryServiceDisabled') }}</strong>
            </div>
            <span class="metric-chip">{{ intelligenceStatus?.source_key || t('notConfigured') }}</span>
          </div>

          <div v-if="intelligenceStatus?.base_url" class="settings-core-grid">
            <span class="hint">API</span>
            <code class="token-link">{{ intelligenceStatus.base_url }}</code>
          </div>

          <label class="settings-switch">
            <input v-model="form.country_detection_auto_enabled" type="checkbox" />
            <span></span>
            <div>
              <strong>{{ t('enableAutoCountryDetection') }}</strong>
              <small>{{ t('enableAutoCountryDetectionHint') }}</small>
            </div>
          </label>

          <label>
            <span class="field-label" for="country-detection-interval">{{ t('countryDetectionInterval') }}</span>
            <div class="settings-inline-field">
              <input
                id="country-detection-interval"
                v-model.number="form.country_detection_interval_minutes"
                class="input"
                max="1440"
                min="5"
                type="number"
              />
              <span class="metric-chip">{{ t('minute') }}</span>
            </div>
          </label>

          <div class="settings-panel-divider"></div>

          <div class="settings-control-grid">
            <label>
              <span class="field-label" for="country-load-threshold">{{ t('countryLoadThreshold') }}</span>
              <div class="settings-inline-field">
                <input
                  id="country-load-threshold"
                  v-model.number="form.mihomo_country_load_min_nodes"
                  class="input"
                  max="20"
                  min="2"
                  type="number"
                />
                <span class="metric-chip">{{ t('nodeCountUnit') }}</span>
              </div>
            </label>
            <label>
              <span class="field-label" for="country-fallback-threshold">{{ t('countryFallbackThreshold') }}</span>
              <div class="settings-inline-field">
                <input
                  id="country-fallback-threshold"
                  v-model.number="form.mihomo_country_fallback_min_nodes"
                  class="input"
                  max="20"
                  min="2"
                  type="number"
                />
                <span class="metric-chip">{{ t('nodeCountUnit') }}</span>
              </div>
            </label>
          </div>

          <p class="compat-copy">{{ t('countryDetectionPrivacy') }}</p>
        </section>

        <section class="settings-console-panel">
          <div class="settings-panel-title">
            <span class="settings-panel-index">05</span>
            <div>
              <strong>{{ t('connectivityDefaults') }}</strong>
              <div class="hint">{{ t('connectivityDefaultsHint') }}</div>
            </div>
          </div>

          <label>
            <span class="field-label" for="connectivity-default-target">{{ t('defaultTestTarget') }}</span>
            <select id="connectivity-default-target" v-model="form.connectivity_default_target" class="input">
              <option value="system_default">{{ t('systemDefaultTarget') }}</option>
              <option value="cloudflare_204">Cloudflare 204</option>
              <option value="google_204">Google 204</option>
            </select>
          </label>

          <label>
            <span class="field-label" for="connectivity-default-rounds">{{ t('defaultTestRounds') }}</span>
            <select id="connectivity-default-rounds" v-model.number="form.connectivity_default_rounds" class="input">
              <option :value="1">{{ t('roundUnit', { count: 1 }) }}</option>
              <option :value="3">{{ t('roundUnit', { count: 3 }) }}</option>
              <option :value="5">{{ t('roundUnit', { count: 5 }) }}</option>
            </select>
          </label>

          <label class="settings-switch">
            <input v-model="form.connectivity_sync_last_latency" type="checkbox" />
            <span></span>
            <div>
              <strong>{{ t('defaultSyncLastLatency') }}</strong>
              <small>{{ t('defaultSyncLastLatencyHint') }}</small>
            </div>
          </label>
        </section>

        <section class="settings-console-panel">
          <div class="settings-panel-title">
            <span class="settings-panel-index">06</span>
            <div>
              <strong>{{ t('publicExportProtection') }}</strong>
              <div class="hint">{{ t('publicExportProtectionHint') }}</div>
            </div>
          </div>

          <div class="settings-control-grid">
            <label>
              <span class="field-label" for="export-cache-ttl">{{ t('exportCacheTtl') }}</span>
              <div class="settings-inline-field">
                <input id="export-cache-ttl" v-model.number="form.public_export_cache_ttl_seconds" class="input" max="300" min="0" type="number" />
                <span class="metric-chip">{{ t('second') }}</span>
              </div>
            </label>
            <label>
              <span class="field-label" for="export-ip-limit">{{ t('exportIpLimit') }}</span>
              <div class="settings-inline-field">
                <input id="export-ip-limit" v-model.number="form.public_export_ip_limit_per_minute" class="input" max="6000" min="10" type="number" />
                <span class="metric-chip">/ min</span>
              </div>
            </label>
            <label>
              <span class="field-label" for="export-global-limit">{{ t('exportGlobalLimit') }}</span>
              <div class="settings-inline-field">
                <input id="export-global-limit" v-model.number="form.public_export_global_limit_per_minute" class="input" max="60000" min="100" type="number" />
                <span class="metric-chip">/ min</span>
              </div>
            </label>
          </div>
          <p class="compat-copy">{{ t('publicExportProtectionCopy') }}</p>
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
