<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { extractApiError } from '../api/client'
import {
  checkForUpdates,
  getVersionInfo,
  type UpdateCheck,
  type VersionInfo,
} from '../api/version'
import { useI18n } from '../i18n'
import { useAuthStore } from '../store/auth'

type SupportValue = 'full' | 'partial' | 'future' | 'none'
type ClientKey =
  | 'xray'
  | 'clash'
  | 'mihomo'
  | 'surge'
  | 'singbox'
  | 'surge3'
  | 'surge2'
  | 'quanx'
  | 'quan'
  | 'loon'
  | 'surfboard'
  | 'mellow'
  | 'clashr'
  | 'ss'
  | 'sssub'
  | 'ssr'
  | 'ssd'
  | 'trojanUri'
  | 'mixed'

const clientColumns: Array<{ key: ClientKey; label: string }> = [
  { key: 'xray', label: 'Xray' },
  { key: 'clash', label: 'Clash' },
  { key: 'mihomo', label: 'Mihomo' },
  { key: 'surge', label: 'Surge 4/5' },
  { key: 'singbox', label: 'sing-box' },
  { key: 'surge3', label: 'Surge 3' },
  { key: 'surge2', label: 'Surge 2' },
  { key: 'quanx', label: 'Quantumult X' },
  { key: 'quan', label: 'Quantumult' },
  { key: 'loon', label: 'Loon' },
  { key: 'surfboard', label: 'Surfboard' },
  { key: 'mellow', label: 'Mellow' },
  { key: 'clashr', label: 'ClashR' },
  { key: 'ss', label: 'SS SIP002' },
  { key: 'sssub', label: 'SS SIP008' },
  { key: 'ssr', label: 'SSR' },
  { key: 'ssd', label: 'SSD' },
  { key: 'trojanUri', label: 'Trojan URI' },
  { key: 'mixed', label: 'Mixed' },
]
function matrixRow(
  protocol: string,
  defaultSupport: SupportValue,
  overrides: Partial<Record<ClientKey, SupportValue>>,
) {
  return {
    protocol,
    support: Object.fromEntries(
      clientColumns.map((client) => [client.key, overrides[client.key] ?? defaultSupport]),
    ) as Record<ClientKey, SupportValue>,
  }
}

const protocolMatrix: Array<{ protocol: string; support: Record<ClientKey, SupportValue> }> = [
  matrixRow('Shadowsocks', 'full', {
    ssr: 'none',
    trojanUri: 'none',
  }),
  matrixRow('Shadowsocks 2022', 'partial', {
    clash: 'full',
    mihomo: 'full',
    singbox: 'full',
    surge: 'none',
    surge3: 'none',
    surge2: 'none',
    ss: 'full',
    sssub: 'full',
    ssr: 'none',
    ssd: 'full',
    trojanUri: 'none',
    mixed: 'full',
  }),
  matrixRow('ShadowsocksR', 'none', {
    ssr: 'full',
  }),
  matrixRow('SOCKS5', 'future', {
    xray: 'full',
    clash: 'full',
    mihomo: 'full',
    surge: 'full',
    singbox: 'full',
    ss: 'none',
    sssub: 'none',
    ssr: 'none',
    ssd: 'none',
    trojanUri: 'none',
    mixed: 'future',
  }),
  matrixRow('HTTP / HTTPS', 'future', {
    clash: 'full',
    mihomo: 'full',
    surge: 'full',
    singbox: 'full',
    ss: 'none',
    sssub: 'none',
    ssr: 'none',
    ssd: 'none',
    trojanUri: 'none',
    mixed: 'future',
  }),
  matrixRow('VMess', 'full', {
    ss: 'none',
    sssub: 'none',
    ssr: 'none',
    ssd: 'none',
    trojanUri: 'none',
  }),
  matrixRow('VLESS', 'full', {
    surge: 'future',
    surge3: 'future',
    surge2: 'future',
    quanx: 'none',
    quan: 'none',
    loon: 'partial',
    surfboard: 'partial',
    ss: 'none',
    sssub: 'none',
    ssr: 'none',
    ssd: 'none',
    trojanUri: 'none',
  }),
  matrixRow('Trojan', 'full', {
    ss: 'none',
    sssub: 'none',
    ssr: 'none',
    ssd: 'none',
    trojanUri: 'full',
  }),
  matrixRow('Hysteria', 'future', {
    clash: 'full',
    mihomo: 'full',
    singbox: 'full',
    surge: 'none',
    surge3: 'none',
    surge2: 'none',
    xray: 'none',
    ss: 'none',
    sssub: 'none',
    ssr: 'none',
    ssd: 'none',
    trojanUri: 'none',
    mixed: 'future',
  }),
  matrixRow('Hysteria2', 'full', {
    quanx: 'partial',
    ss: 'none',
    sssub: 'none',
    ssr: 'none',
    ssd: 'none',
    trojanUri: 'none',
  }),
  matrixRow('TUIC', 'full', {
    xray: 'none',
    quanx: 'none',
    ss: 'none',
    sssub: 'none',
    ssr: 'none',
    ssd: 'none',
    trojanUri: 'none',
  }),
  matrixRow('WireGuard', 'full', {
    xray: 'none',
    quanx: 'none',
    ss: 'none',
    sssub: 'none',
    ssr: 'none',
    ssd: 'none',
    trojanUri: 'none',
  }),
  matrixRow('Snell', 'future', {
    xray: 'none',
    singbox: 'none',
    quanx: 'none',
    quan: 'none',
    ss: 'none',
    sssub: 'none',
    ssr: 'none',
    ssd: 'none',
    trojanUri: 'none',
    mixed: 'future',
  }),
  matrixRow('AnyTLS', 'none', {
    clash: 'full',
    mihomo: 'full',
    singbox: 'full',
    mellow: 'full',
    clashr: 'full',
    mixed: 'full',
  }),
  matrixRow('NaiveProxy', 'none', {
    singbox: 'future',
    mixed: 'future',
  }),
  matrixRow('SSH', 'none', {
    surge: 'future',
    surge3: 'future',
    surge2: 'future',
    singbox: 'future',
    mixed: 'future',
  }),
  matrixRow('Juicity', 'none', {
    mixed: 'future',
  }),
]
const UPDATE_CACHE_KEY = 'sublinkx_about_update_cache'
const SIX_HOURS_MS = 6 * 60 * 60 * 1000

interface AboutUpdateCache {
  date: string
  firstAutoAt: number | null
  secondAutoAt: number | null
  lastCheckedAt: number | null
  version: string | null
  updateInfo: UpdateCheck | null
}

const { t, locale } = useI18n()
const auth = useAuthStore()
const loading = ref(false)
const checkingUpdate = ref(false)
const errorMessage = ref('')
const updateMessage = ref('')
const versionInfo = ref<VersionInfo | null>(null)
const updateInfo = ref<UpdateCheck | null>(null)
const updateCache = ref<AboutUpdateCache | null>(null)

const updateBadgeClass = computed(() => {
  if (!updateInfo.value) {
    return 'status-badge-neutral'
  }
  if (!updateInfo.value.checked) {
    return 'status-badge-muted'
  }
  return updateInfo.value.update_available ? 'status-badge-warn' : 'status-badge-ok'
})

const updateStatusText = computed(() => {
  if (!updateInfo.value) {
    return t('notChecked')
  }
  if (!updateInfo.value.checked) {
    return t('updateCheckFailed')
  }
  return updateInfo.value.update_available ? t('updateAvailable') : t('alreadyLatest')
})

const checkingAny = computed(() => loading.value || checkingUpdate.value)
const userLabel = computed(() => auth.user?.nickname || auth.user?.username || t('unknown'))
const developerName = computed(() => versionInfo.value?.developer?.name ?? t('unknown'))
const developerUrl = computed(() => versionInfo.value?.developer?.url ?? versionInfo.value?.repository ?? '#')
const runtimeModeText = computed(() => {
  if (versionInfo.value?.runtime_mode === 'docker') {
    return t('runtimeModeDocker')
  }
  if (versionInfo.value?.runtime_mode === 'local') {
    return t('runtimeModeLocal')
  }
  if (versionInfo.value?.environment === 'development') {
    return t('runtimeModeLocal')
  }
  return versionInfo.value?.runtime_mode ?? t('unknown')
})

const releaseDateText = computed(() => {
  const publishedAt = updateInfo.value?.published_at
  if (!publishedAt) {
    return t('unknown')
  }

  return new Intl.DateTimeFormat(locale.value, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(publishedAt))
})

const serverTimeText = computed(() => {
  if (!versionInfo.value?.server_time) {
    return t('unknown')
  }

  return new Intl.DateTimeFormat(locale.value, {
    dateStyle: 'medium',
    timeStyle: 'medium',
  }).format(new Date(versionInfo.value.server_time))
})

function todayKey() {
  return new Intl.DateTimeFormat('en-CA', {
    timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone,
  }).format(new Date())
}

function readUpdateCache(): AboutUpdateCache | null {
  try {
    const raw = localStorage.getItem(UPDATE_CACHE_KEY)
    return raw ? JSON.parse(raw) as AboutUpdateCache : null
  } catch {
    return null
  }
}

function writeUpdateCache(nextCache: AboutUpdateCache) {
  updateCache.value = nextCache
  localStorage.setItem(UPDATE_CACHE_KEY, JSON.stringify(nextCache))
}

function applyCachedChecks(currentVersion: string) {
  const cache = readUpdateCache()
  updateCache.value = cache
  if (!cache || cache.date !== todayKey() || cache.version !== currentVersion) {
    return
  }
  updateInfo.value = cache.updateInfo
}

function shouldAutoCheck(currentVersion: string) {
  const now = Date.now()
  const today = todayKey()
  const cache = updateCache.value

  if (!cache || cache.date !== today || cache.version !== currentVersion || !cache.firstAutoAt) {
    return true
  }

  return !cache.secondAutoAt && now - cache.firstAutoAt >= SIX_HOURS_MS
}

function persistCheckResults(isAuto: boolean) {
  const now = Date.now()
  const today = todayKey()
  const currentVersion = versionInfo.value?.version ?? null
  const previous = updateCache.value
  const isSameAutoDay = previous?.date === today && previous.version === currentVersion
  const firstAutoAt = isAuto
    ? (isSameAutoDay ? previous?.firstAutoAt ?? now : now)
    : (isSameAutoDay ? previous?.firstAutoAt ?? null : null)
  const secondAutoAt = isAuto && firstAutoAt !== now
    ? now
    : (isSameAutoDay ? previous?.secondAutoAt ?? null : null)

  writeUpdateCache({
    date: today,
    firstAutoAt,
    secondAutoAt,
    lastCheckedAt: now,
    version: currentVersion,
    updateInfo: updateInfo.value,
  })
}

function supportText(value: string) {
  if (value === 'full') {
    return t('supportFull')
  }
  if (value === 'partial') {
    return t('supportPartial')
  }
  if (value === 'future') {
    return t('supportFuture')
  }
  return t('supportNone')
}

function supportClass(value: string) {
  if (value === 'full') {
    return 'matrix-support-full'
  }
  if (value === 'partial') {
    return 'matrix-support-partial'
  }
  if (value === 'future') {
    return 'matrix-support-future'
  }
  return 'matrix-support-none'
}

const uptimeText = computed(() => {
  const seconds = versionInfo.value?.uptime_seconds
  if (seconds === undefined) {
    return t('unknown')
  }

  const days = Math.floor(seconds / 86400)
  const hours = Math.floor((seconds % 86400) / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  const remainSeconds = seconds % 60
  const parts = []
  if (days > 0) {
    parts.push(t('daysShort', { count: days }))
  }
  if (hours > 0 || parts.length > 0) {
    parts.push(t('hoursShort', { count: hours }))
  }
  if (minutes > 0 || parts.length > 0) {
    parts.push(t('minutesShort', { count: minutes }))
  }
  parts.push(t('secondsShort', { count: remainSeconds }))
  return parts.join(' ')
})

async function load() {
  loading.value = true
  errorMessage.value = ''
  try {
    versionInfo.value = await getVersionInfo()
    applyCachedChecks(versionInfo.value.version)
    if (shouldAutoCheck(versionInfo.value.version)) {
      void runFullUpdateCheck(false, true)
    }
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    loading.value = false
  }
}

async function runFullUpdateCheck(showError = true, isAuto = false) {
  updateMessage.value = ''
  checkingUpdate.value = true
  try {
    updateInfo.value = await checkForUpdates()
  } catch (error) {
    updateInfo.value = {
      checked: false,
      update_available: false,
      latest_version: null,
      latest_url: 'https://github.com/jojhaa/sublinkx-rs/releases',
      release_name: null,
      published_at: null,
      error: extractApiError(error),
    }
    if (showError) {
      updateMessage.value = updateInfo.value.error ?? t('updateCheckFailed')
    }
  } finally {
    persistCheckResults(isAuto)
    checkingUpdate.value = false
  }
}

onMounted(load)
</script>

<template>
  <section class="stack">
    <header class="page-header">
      <div>
        <span class="eyebrow">{{ t('aboutEyebrow') }}</span>
        <h2 class="page-title">{{ t('aboutTitle') }}</h2>
        <p class="page-copy">{{ t('aboutCopy') }}</p>
      </div>
      <button class="button button-ghost" type="button" :disabled="checkingAny" @click="runFullUpdateCheck()">
        {{ checkingAny ? t('checkingUpdate') : t('checkUpdate') }}
      </button>
    </header>

    <div v-if="errorMessage" class="error-banner">{{ errorMessage }}</div>
    <div v-if="updateMessage" class="error-banner">{{ updateMessage }}</div>

    <div class="about-grid">
      <article class="settings-console about-hero-card">
        <div class="settings-console-header">
          <div>
            <span class="hint">{{ t('aboutProjectHint') }}</span>
            <h3>SublinkX RS</h3>
            <p class="card-copy">{{ t('aboutProjectCopy') }}</p>
          </div>
          <span class="settings-console-mark">RUST + VUE</span>
        </div>

        <div class="about-meta-grid">
          <div>
            <span class="hint">{{ t('currentServerVersion') }}</span>
            <strong>{{ versionInfo?.version ?? t('unknown') }}</strong>
          </div>
          <div>
            <span class="hint">{{ t('apiVersion') }}</span>
            <strong>{{ versionInfo?.api_version ?? t('unknown') }}</strong>
          </div>
          <div>
            <span class="hint">{{ t('runtimeEnvironment') }}</span>
            <strong>{{ versionInfo?.environment ?? t('unknown') }}</strong>
          </div>
          <div>
            <span class="hint">{{ t('runtimeMode') }}</span>
            <strong>{{ runtimeModeText }}</strong>
          </div>
          <div>
            <span class="hint">{{ t('license') }}</span>
            <strong>{{ versionInfo?.license ?? 'AGPL-3.0-or-later' }}</strong>
          </div>
          <div>
            <span class="hint">{{ t('serverTimezone') }}</span>
            <strong>{{ versionInfo?.server_timezone ?? t('unknown') }}</strong>
          </div>
          <div>
            <span class="hint">{{ t('serverTime') }}</span>
            <strong>{{ serverTimeText }}</strong>
          </div>
          <div>
            <span class="hint">{{ t('serverUptime') }}</span>
            <strong>{{ uptimeText }}</strong>
          </div>
          <div>
            <span class="hint">{{ t('serverSystem') }}</span>
            <strong>{{ versionInfo?.system.display ?? t('unknown') }}</strong>
          </div>
          <div>
            <span class="hint">{{ t('serverArch') }}</span>
            <strong>{{ versionInfo?.system.arch ?? t('unknown') }}</strong>
          </div>
          <div>
            <span class="hint">{{ t('currentUsername') }}</span>
            <strong>{{ userLabel }}</strong>
          </div>
          <div>
            <span class="hint">{{ t('developer') }}</span>
            <strong>
              <a class="token-link" :href="developerUrl" target="_blank" rel="noreferrer">
                {{ developerName }}
              </a>
            </strong>
          </div>
        </div>
      </article>

      <article class="card about-update-card">
        <div class="about-card-title">
          <div>
            <span class="hint">{{ t('updateDetection') }}</span>
            <h3>{{ t('releaseChannel') }}</h3>
          </div>
          <span class="status-badge" :class="updateBadgeClass">{{ updateStatusText }}</span>
        </div>

        <div class="about-version-line">
          <span>{{ t('latestVersion') }}</span>
          <strong>{{ updateInfo?.latest_version ?? t('unknown') }}</strong>
        </div>
        <div class="about-version-line">
          <span>{{ t('releaseTime') }}</span>
          <strong>{{ releaseDateText }}</strong>
        </div>

        <p v-if="checkingUpdate" class="card-copy">
          {{ t('updateCheckingCopy') }}
        </p>
        <p v-else-if="updateInfo?.error" class="card-copy">
          {{ updateInfo.error }}
        </p>
        <p v-else-if="!updateInfo" class="card-copy">{{ t('updateNotCheckedCopy') }}</p>
        <p v-else class="card-copy">
          {{ updateInfo.update_available ? t('updateAvailableCopy') : t('alreadyLatestCopy') }}
        </p>

        <div class="inline-actions">
          <a
            class="button button-accent"
            :href="updateInfo?.latest_url ?? versionInfo?.repository ?? 'https://github.com/jojhaa/sublinkx-rs/releases'"
            target="_blank"
            rel="noreferrer"
          >
            {{ t('openReleasePage') }}
          </a>
          <a
            class="button button-ghost"
            :href="versionInfo?.repository ?? 'https://github.com/jojhaa/sublinkx-rs'"
            target="_blank"
            rel="noreferrer"
          >
            GitHub
          </a>
        </div>
      </article>

      <article class="card wide-card about-matrix-card">
        <div class="about-card-title">
          <div>
            <span class="hint">{{ t('protocolClientSupport') }}</span>
            <h3>{{ t('supportMatrix') }}</h3>
          </div>
          <span class="status-badge status-badge-neutral">{{ t('clientRenderer') }}</span>
        </div>

        <div class="matrix-table-wrap">
          <table class="matrix-table">
            <thead>
              <tr>
                <th>{{ t('protocol') }}</th>
                <th v-for="client in clientColumns" :key="client.key">{{ client.label }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="row in protocolMatrix" :key="row.protocol">
                <th>{{ row.protocol }}</th>
                <td v-for="client in clientColumns" :key="`${row.protocol}-${client.key}`">
                  <span class="matrix-support" :class="supportClass(row.support[client.key])">
                    {{ supportText(row.support[client.key]) }}
                  </span>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </article>

      <article class="card wide-card">
        <div class="hint">{{ t('upstreamThanks') }}</div>
        <p class="card-copy">{{ t('upstreamThanksCopy') }}</p>
      </article>
    </div>
  </section>
</template>
