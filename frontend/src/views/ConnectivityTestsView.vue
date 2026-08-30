<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { onBeforeRouteLeave } from 'vue-router'
import { extractApiError, extractApiErrorCode } from '../api/client'
import {
  cancelConnectivityTest,
  listConnectivityTestPresets,
  streamConnectivityTest,
  type ConnectivityTestEvent,
  type ConnectivityTestPreset,
} from '../api/connectivityTests'
import { listNodeGroups, type GroupItem } from '../api/groups'
import { cancelAutoLatency, listNodes, type NodeItem } from '../api/nodes'
import { getSettings } from '../api/settings'
import { useI18n } from '../i18n'

type GroupFilter = number | 'all' | 'none'
type EnabledFilter = 'all' | 'enabled' | 'paused'
type TestRound = 1 | 3 | 5

interface SampleView {
  round: number
  status: 'ok' | 'error' | 'cancelled'
  latency_ms: number | null
  message: string | null
}

interface NodeResultView {
  samples: SampleView[]
  summary: Extract<ConnectivityTestEvent, { type: 'node_completed' }> | null
}

const { t } = useI18n()
const nodes = ref<NodeItem[]>([])
const groups = ref<GroupItem[]>([])
const presets = ref<ConnectivityTestPreset[]>([])
const loading = ref(false)
const running = ref(false)
const stopping = ref(false)
const selectedIds = ref<number[]>([])
const groupFilter = ref<GroupFilter>('all')
const enabledFilter = ref<EnabledFilter>('enabled')
const search = ref('')
const targetId = ref('system_default')
const rounds = ref<TestRound>(3)
const syncLastLatency = ref(true)
const results = ref<Record<number, NodeResultView>>({})
const jobMeta = ref<Extract<ConnectivityTestEvent, { type: 'job_started' }> | null>(null)
const jobComplete = ref<Extract<ConnectivityTestEvent, { type: 'job_completed' }> | null>(null)
const errorMessage = ref('')
const successMessage = ref('')
const candidateTotal = ref(0)
let activeController: AbortController | null = null
let leavingPage = false

const groupNames = computed(() => new Map(groups.value.map((group) => [group.id, group.name])))
const filteredNodes = computed(() => {
  const query = search.value.trim().toLowerCase()
  return nodes.value.filter((node) => {
    if (groupFilter.value === 'none' && node.group_id !== null) return false
    if (typeof groupFilter.value === 'number' && node.group_id !== groupFilter.value) return false
    if (enabledFilter.value === 'enabled' && !node.enabled) return false
    if (enabledFilter.value === 'paused' && node.enabled) return false
    if (!query) return true
    return [node.name, node.protocol, node.server].some((value) => value.toLowerCase().includes(query))
  })
})
const selectedNodes = computed(() => {
  const selected = new Set(selectedIds.value)
  return nodes.value.filter((node) => selected.has(node.id))
})
const selectedTarget = computed(() => presets.value.find((preset) => preset.id === targetId.value) ?? null)
const isSystemTarget = computed(() => selectedTarget.value?.is_system_default === true)
const resultRows = computed(() =>
  selectedNodes.value.map((node) => ({
    node,
    result: results.value[node.id] ?? { samples: [], summary: null },
  })),
)
const completedSamples = computed(() =>
  Object.values(results.value).reduce((total, result) => total + result.samples.length, 0),
)
const totalSamples = computed(() => selectedIds.value.length * rounds.value)
const progressPercent = computed(() => {
  if (totalSamples.value === 0) return 0
  return Math.min(100, Math.round((completedSamples.value / totalSamples.value) * 100))
})
const completedSummaries = computed(() =>
  Object.values(results.value)
    .map((result) => result.summary)
    .filter((summary): summary is NonNullable<typeof summary> => summary !== null),
)
const successfulCount = computed(() => completedSummaries.value.filter((summary) => summary.succeeded > 0).length)
const failedCount = computed(() => completedSummaries.value.filter((summary) => summary.succeeded === 0).length)
const averageLatency = computed(() => {
  const values = completedSummaries.value
    .map((summary) => summary.average_ms)
    .filter((value): value is number => value !== null)
  if (!values.length) return null
  return Math.round(values.reduce((total, value) => total + value, 0) / values.length)
})

function groupName(groupId: number | null) {
  if (groupId === null) return t('ungrouped')
  return groupNames.value.get(groupId) ?? t('groupFallback', { id: groupId })
}

function targetName(preset: ConnectivityTestPreset) {
  return preset.is_system_default ? t('systemDefaultTarget') : preset.name
}

function isSelected(id: number) {
  return selectedIds.value.includes(id)
}

function toggleNode(id: number) {
  if (running.value) return
  if (isSelected(id)) {
    selectedIds.value = selectedIds.value.filter((value) => value !== id)
    return
  }
  if (selectedIds.value.length >= 200) {
    errorMessage.value = t('selectionLimitReached')
    return
  }
  selectedIds.value = [...selectedIds.value, id]
}

function selectFiltered() {
  if (running.value) return
  const merged = [...selectedIds.value]
  for (const node of filteredNodes.value) {
    if (!merged.includes(node.id) && merged.length < 200) merged.push(node.id)
  }
  selectedIds.value = merged
  if (filteredNodes.value.some((node) => !merged.includes(node.id))) {
    errorMessage.value = t('selectionLimitReached')
  }
}

function clearSelection() {
  if (running.value) return
  selectedIds.value = []
  results.value = {}
  jobMeta.value = null
  jobComplete.value = null
}

function sampleFor(result: NodeResultView, round: number) {
  return result.samples.find((sample) => sample.round === round) ?? null
}

function statusLabel(status?: string) {
  if (!status) return t('queued')
  if (status === 'ok') return t('supportFull')
  if (status === 'partial') return t('partial')
  if (status === 'cancelled') return t('cancelled')
  return t('failed')
}

function resultStatusClass(status?: string) {
  if (status === 'ok') return 'connectivity-status connectivity-status-ok'
  if (status === 'partial') return 'connectivity-status connectivity-status-partial'
  if (status === 'error') return 'connectivity-status connectivity-status-error'
  if (status === 'cancelled') return 'connectivity-status connectivity-status-cancelled'
  return 'connectivity-status'
}

function formatLatency(value: number | null) {
  return value === null ? '—' : `${value} ms`
}

function applyEvent(event: ConnectivityTestEvent) {
  if (event.type === 'job_started') {
    jobMeta.value = event
    return
  }
  if (event.type === 'sample_completed') {
    const current = results.value[event.id] ?? { samples: [], summary: null }
    const samples = current.samples.filter((sample) => sample.round !== event.round)
    samples.push({
      round: event.round,
      status: event.status,
      latency_ms: event.latency_ms,
      message: event.message,
    })
    samples.sort((left, right) => left.round - right.round)
    results.value = {
      ...results.value,
      [event.id]: { ...current, samples },
    }
    return
  }
  if (event.type === 'node_completed') {
    const current = results.value[event.id] ?? { samples: [], summary: null }
    results.value = {
      ...results.value,
      [event.id]: { ...current, summary: event },
    }
    return
  }
  jobComplete.value = event
  stopping.value = false
  successMessage.value = event.cancelled
    ? t('connectivityCancelled')
    : t('connectivityComplete', { ok: event.succeeded, failed: event.failed })
}

async function executeStream() {
  activeController = new AbortController()
  await streamConnectivityTest(
    {
      ids: selectedIds.value,
      target_id: targetId.value,
      rounds: rounds.value,
      sync_last_latency: syncLastLatency.value && isSystemTarget.value,
    },
    applyEvent,
    activeController.signal,
  )
}

async function executeWithAutoConflict() {
  try {
    await executeStream()
  } catch (error) {
    if (extractApiErrorCode(error) !== 'latency_auto_running') throw error
    if (!window.confirm(t('confirmStopAutoLatency'))) return
    await cancelAutoLatency()
    let lastError: unknown = error
    for (let attempt = 0; attempt < 12; attempt += 1) {
      await new Promise((resolve) => window.setTimeout(resolve, 300))
      try {
        await executeStream()
        return
      } catch (retryError) {
        if (extractApiErrorCode(retryError) !== 'latency_auto_running') throw retryError
        lastError = retryError
      }
    }
    throw lastError
  }
}

async function startTest() {
  if (!selectedIds.value.length) {
    errorMessage.value = t('selectAtLeastOneNode')
    return
  }
  errorMessage.value = ''
  successMessage.value = ''
  results.value = Object.fromEntries(selectedIds.value.map((id) => [id, { samples: [], summary: null }]))
  jobMeta.value = null
  jobComplete.value = null
  running.value = true
  stopping.value = false
  try {
    await executeWithAutoConflict()
  } catch (error) {
    if (!(error instanceof DOMException && error.name === 'AbortError') && !leavingPage) {
      errorMessage.value = extractApiError(error)
    }
  } finally {
    running.value = false
    stopping.value = false
    activeController = null
  }
}

async function stopTest() {
  if (!running.value || stopping.value) return
  stopping.value = true
  try {
    await cancelConnectivityTest()
  } catch (error) {
    errorMessage.value = extractApiError(error)
    stopping.value = false
  }
}

async function load() {
  loading.value = true
  errorMessage.value = ''
  try {
    const [nodeResponse, groupResponse, presetResponse, settingsResponse] = await Promise.all([
      listNodes({ page: 1, page_size: 1000 }),
      listNodeGroups(),
      listConnectivityTestPresets(),
      getSettings(),
    ])
    nodes.value = nodeResponse.data
    candidateTotal.value = nodeResponse.pagination?.total ?? nodeResponse.data.length
    groups.value = groupResponse.data
    presets.value = presetResponse.data
    targetId.value = settingsResponse.data.connectivity_default_target
    rounds.value = settingsResponse.data.connectivity_default_rounds
    syncLastLatency.value = settingsResponse.data.connectivity_sync_last_latency
    if (!presets.value.some((preset) => preset.id === targetId.value)) {
      targetId.value = presets.value[0]?.id ?? 'system_default'
    }
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    loading.value = false
  }
}

function cancelOnLeave() {
  if (!running.value) return
  leavingPage = true
  void cancelConnectivityTest().catch(() => undefined)
  activeController?.abort()
}

onBeforeRouteLeave(() => cancelOnLeave())
onBeforeUnmount(() => cancelOnLeave())
onMounted(load)
</script>

<template>
  <section class="content-stack connectivity-page">
    <header class="page-header connectivity-page-header">
      <div>
        <span class="eyebrow">{{ t('connectivityEyebrow') }}</span>
        <h2 class="page-title">{{ t('connectivityTitle') }}</h2>
        <p class="page-copy">{{ t('connectivityCopy') }}</p>
      </div>
      <div class="connectivity-run-state" :class="{ active: running }">
        <span class="connectivity-run-light"></span>
        <div>
          <small>MIHOMO SESSION</small>
          <strong>{{ running ? t('testRunning') : t('testIdle') }}</strong>
        </div>
      </div>
    </header>

    <div v-if="errorMessage" class="alert alert-error">{{ errorMessage }}</div>
    <div v-if="successMessage" class="alert alert-success">{{ successMessage }}</div>

    <section class="connectivity-console">
      <div class="connectivity-console-main">
        <div class="connectivity-section-heading">
          <div>
            <span>01 / TARGET</span>
            <h3>{{ t('targetPreset') }}</h3>
          </div>
          <code>{{ selectedTarget?.url ?? '—' }}</code>
        </div>

        <div class="connectivity-target-grid">
          <label
            v-for="preset in presets"
            :key="preset.id"
            class="connectivity-target-card"
            :class="{ selected: targetId === preset.id }"
          >
            <input v-model="targetId" :disabled="running" :value="preset.id" type="radio" />
            <span class="connectivity-target-index">{{ String(presets.indexOf(preset) + 1).padStart(2, '0') }}</span>
            <strong>{{ targetName(preset) }}</strong>
            <small>{{ preset.url }}</small>
          </label>
        </div>

        <div class="connectivity-control-row">
          <div>
            <span class="field-label">{{ t('testRounds') }}</span>
            <div class="connectivity-round-switch" role="group" :aria-label="t('testRounds')">
              <button
                v-for="option in ([1, 3, 5] as const)"
                :key="option"
                :class="{ active: rounds === option }"
                :disabled="running"
                type="button"
                @click="rounds = option"
              >
                {{ t('roundUnit', { count: option }) }}
              </button>
            </div>
          </div>

          <label class="connectivity-sync-toggle" :class="{ disabled: !isSystemTarget }">
            <input v-model="syncLastLatency" :disabled="running || !isSystemTarget" type="checkbox" />
            <span></span>
            <div>
              <strong>{{ t('syncLastLatency') }}</strong>
              <small>{{ t('syncLastLatencyHint') }}</small>
            </div>
          </label>
        </div>
      </div>

      <aside class="connectivity-meter-panel">
        <div class="connectivity-progress-ring" :style="{ '--progress': `${progressPercent * 3.6}deg` }">
          <div>
            <strong>{{ progressPercent }}%</strong>
            <span>{{ t('testProgress') }}</span>
          </div>
        </div>
        <div class="connectivity-live-metrics">
          <div><span>{{ t('successfulNodes') }}</span><strong>{{ successfulCount }}</strong></div>
          <div><span>{{ t('failedNodes') }}</span><strong>{{ failedCount }}</strong></div>
          <div><span>{{ t('averageLatency') }}</span><strong>{{ formatLatency(averageLatency) }}</strong></div>
          <div><span>CONCURRENCY</span><strong>{{ jobMeta?.concurrency ?? '—' }}</strong></div>
        </div>
      </aside>
    </section>

    <section class="card connectivity-node-picker">
      <div class="connectivity-picker-head">
        <div>
          <span class="eyebrow">02 / NODE SET</span>
          <h3>{{ t('selectedNodeCount', { count: selectedIds.length }) }}</h3>
          <p>{{ t('maxConnectivityNodes') }}</p>
        </div>
        <div class="inline-actions">
          <button class="button button-ghost" :disabled="running" type="button" @click="selectFiltered">
            {{ t('selectFiltered') }}
          </button>
          <button class="button button-ghost" :disabled="running || !selectedIds.length" type="button" @click="clearSelection">
            {{ t('clearSelection') }}
          </button>
        </div>
      </div>

      <div class="connectivity-filter-grid">
        <select v-model="groupFilter" class="select" :disabled="running">
          <option value="all">{{ t('allGroups') }}</option>
          <option value="none">{{ t('ungrouped') }}</option>
          <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option>
        </select>
        <select v-model="enabledFilter" class="select" :disabled="running">
          <option value="all">{{ t('allStatuses') }}</option>
          <option value="enabled">{{ t('enabledNodes') }}</option>
          <option value="paused">{{ t('pausedNodes') }}</option>
        </select>
        <input v-model.trim="search" class="input" :disabled="running" :placeholder="t('searchNodes')" />
      </div>

      <div v-if="candidateTotal > nodes.length" class="hint connectivity-limit-note">{{ t('candidateLimitNotice') }}</div>
      <div v-if="loading" class="empty-state">{{ t('refreshing') }}</div>
      <div v-else-if="!filteredNodes.length" class="empty-state">{{ t('noNodesMatch') }}</div>
      <div v-else class="connectivity-node-grid">
        <label
          v-for="node in filteredNodes"
          :key="node.id"
          class="connectivity-node-option"
          :class="{ selected: isSelected(node.id), paused: !node.enabled }"
        >
          <input
            :checked="isSelected(node.id)"
            :disabled="running"
            type="checkbox"
            @change="toggleNode(node.id)"
          />
          <span class="connectivity-node-check"></span>
          <div>
            <strong>{{ node.name }}</strong>
            <small>{{ node.protocol }} · {{ groupName(node.group_id) }}</small>
          </div>
          <span class="connectivity-node-state">{{ node.enabled ? 'ON' : 'PAUSED' }}</span>
        </label>
      </div>

      <div class="connectivity-action-rail">
        <div>
          <strong>{{ t('selectedNodeCount', { count: selectedIds.length }) }}</strong>
          <span>{{ selectedTarget ? targetName(selectedTarget) : '—' }} · {{ t('roundUnit', { count: rounds }) }}</span>
        </div>
        <button
          v-if="!running"
          class="button button-accent connectivity-start-button"
          :disabled="loading || !selectedIds.length"
          type="button"
          @click="startTest"
        >
          {{ t('startConnectivityTest') }}
        </button>
        <button
          v-else
          class="button button-danger connectivity-start-button"
          :disabled="stopping"
          type="button"
          @click="stopTest"
        >
          {{ stopping ? t('stoppingTest') : t('stopConnectivityTest') }}
        </button>
      </div>
    </section>

    <section class="card connectivity-results-panel">
      <div class="connectivity-picker-head">
        <div>
          <span class="eyebrow">03 / LIVE MATRIX</span>
          <h3>{{ t('liveResults') }}</h3>
          <p>{{ t('liveResultsCopy') }}</p>
        </div>
        <span class="connectivity-sample-counter">{{ completedSamples }} / {{ totalSamples }}</span>
      </div>

      <div v-if="!resultRows.length" class="empty-state">{{ t('selectAtLeastOneNode') }}</div>
      <div v-else class="connectivity-result-list">
        <article v-for="row in resultRows" :key="row.node.id" class="connectivity-result-row">
          <div class="connectivity-result-identity">
            <span :class="resultStatusClass(row.result.summary?.status)">
              {{ statusLabel(row.result.summary?.status) }}
            </span>
            <div>
              <strong>{{ row.node.name }}</strong>
              <small>{{ row.node.protocol }} · {{ groupName(row.node.group_id) }}</small>
            </div>
          </div>

          <div class="connectivity-sample-track" :aria-label="t('roundSamples')">
            <div
              v-for="round in rounds"
              :key="round"
              class="connectivity-sample-cell"
              :class="sampleFor(row.result, round)?.status ?? 'queued'"
              :title="sampleFor(row.result, round)?.message ?? ''"
            >
              <span>R{{ round }}</span>
              <strong>{{ formatLatency(sampleFor(row.result, round)?.latency_ms ?? null) }}</strong>
            </div>
          </div>

          <div class="connectivity-stat-strip">
            <div><span>{{ t('minimum') }}</span><strong>{{ formatLatency(row.result.summary?.min_ms ?? null) }}</strong></div>
            <div><span>{{ t('average') }}</span><strong>{{ formatLatency(row.result.summary?.average_ms ?? null) }}</strong></div>
            <div><span>{{ t('maximum') }}</span><strong>{{ formatLatency(row.result.summary?.max_ms ?? null) }}</strong></div>
            <div><span>{{ t('jitter') }}</span><strong>{{ formatLatency(row.result.summary?.jitter_ms ?? null) }}</strong></div>
            <div><span>{{ t('successRate') }}</span><strong>{{ row.result.summary?.success_rate ?? 0 }}%</strong></div>
          </div>

          <p v-if="row.result.summary?.message" class="connectivity-result-message">
            {{ row.result.summary.message }}
          </p>
        </article>
      </div>
    </section>
  </section>
</template>
