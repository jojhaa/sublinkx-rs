<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { onBeforeRouteLeave } from 'vue-router'
import { extractApiError, extractApiErrorCode } from '../api/client'
import { listNodeGroups, type GroupItem } from '../api/groups'
import {
  cancelNodeIpProbes,
  getNodeIpIntelligenceStatus,
  listNodeIpProbes,
  refreshNodeIpIntelligence,
  streamNodeIpProbes,
  type NodeIpProbeEvent,
  type NodeIpProbeItem,
} from '../api/nodeIpProbes'
import { cancelAutoLatency, listNodes, type NodeItem } from '../api/nodes'
import { useI18n } from '../i18n'
import { formatCountryName } from '../utils/country'

type GroupFilter = number | 'all' | 'none'
type EnabledFilter = 'all' | 'enabled' | 'paused'

const { t } = useI18n()
const nodes = ref<NodeItem[]>([])
const groups = ref<GroupItem[]>([])
const probes = ref<Record<number, NodeIpProbeItem>>({})
const selectedIds = ref<number[]>([])
const groupFilter = ref<GroupFilter>('all')
const enabledFilter = ref<EnabledFilter>('enabled')
const search = ref('')
const loading = ref(false)
const running = ref(false)
const stopping = ref(false)
const refreshingIntelligence = ref(false)
const intelligenceEnabled = ref(false)
const intelligenceBaseUrl = ref<string | null>(null)
const completed = ref(0)
const provider = ref('ipify')
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
const successfulCount = computed(() => selectedIds.value.filter((id) => probes.value[id]?.status === 'ok').length)
const failedCount = computed(() => selectedIds.value.filter((id) => probes.value[id]?.status === 'error').length)
const progressPercent = computed(() => {
  if (!selectedIds.value.length) return 0
  return Math.min(100, Math.round((completed.value / selectedIds.value.length) * 100))
})

function groupName(groupId: number | null) {
  if (groupId === null) return t('ungrouped')
  return groupNames.value.get(groupId) ?? t('groupFallback', { id: groupId })
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
  completed.value = 0
}

function resultStatus(id: number) {
  if (running.value && selectedIds.value.includes(id) && !probes.value[id]) return t('queued')
  const status = probes.value[id]?.status
  if (status === 'ok') return t('ipProbeSuccess')
  if (status === 'error') return t('failed')
  return t('ipProbeNever')
}

function applyEvent(event: NodeIpProbeEvent) {
  if (event.type === 'job_started') {
    provider.value = event.provider
    return
  }
  if (event.type === 'node_completed') {
    const previous = probes.value[event.id]
    probes.value = {
      ...probes.value,
      [event.id]: {
        node_id: event.id,
        status: event.status,
        ip: event.ip,
        ip_version: event.ip_version,
        country_code: event.country_code,
        country_name: event.country_name,
        country_source: event.country_code ? (previous?.country_source ?? null) : null,
        intelligence_status: previous?.intelligence_status ?? null,
        intelligence_message: previous?.intelligence_message ?? null,
        message: event.message,
        probed_at: event.probed_at,
        country_updated_at: event.country_code ? (previous?.country_updated_at ?? null) : null,
        intelligence_updated_at: previous?.intelligence_updated_at ?? null,
        updated_at: event.probed_at,
      },
    }
    completed.value += 1
    return
  }
  if (event.type === 'intelligence_updated') {
    const previous = probes.value[event.id]
    if (!previous) return
    probes.value = {
      ...probes.value,
      [event.id]: {
        ...previous,
        country_code: event.country_code,
        country_name: event.country_name,
        country_source: event.country_source,
        intelligence_status: event.intelligence_status,
        intelligence_message: event.intelligence_message,
        intelligence_updated_at: event.intelligence_updated_at,
      },
    }
    return
  }
  stopping.value = false
  successMessage.value = event.cancelled
    ? t('ipProbeCancelled')
    : t('ipProbeComplete', { ok: event.succeeded, failed: event.failed })
}

async function executeStream() {
  activeController = new AbortController()
  await streamNodeIpProbes(selectedIds.value, applyEvent, activeController.signal)
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

async function startProbe() {
  if (!selectedIds.value.length) {
    errorMessage.value = t('selectAtLeastOneNode')
    return
  }
  errorMessage.value = ''
  successMessage.value = ''
  completed.value = 0
  for (const id of selectedIds.value) delete probes.value[id]
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

async function stopProbe() {
  if (!running.value || stopping.value) return
  stopping.value = true
  try {
    await cancelNodeIpProbes()
  } catch (error) {
    errorMessage.value = extractApiError(error)
    stopping.value = false
  }
}

async function refreshIntelligence() {
  const ids = selectedIds.value.filter((id) => probes.value[id]?.status === 'ok' && probes.value[id]?.ip)
  if (!ids.length || refreshingIntelligence.value) return
  refreshingIntelligence.value = true
  errorMessage.value = ''
  successMessage.value = ''
  try {
    const response = await refreshNodeIpIntelligence(ids)
    probes.value = {
      ...probes.value,
      ...Object.fromEntries(response.data.map((item) => [item.node_id, item])),
    }
    successMessage.value = t('ipIntelligenceRefreshComplete', {
      updated: response.updated,
      pending: response.pending,
      failed: response.failed,
    })
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    refreshingIntelligence.value = false
  }
}

function intelligenceLabel(item: NodeIpProbeItem | undefined) {
  if (!intelligenceEnabled.value) return t('ipIntelligenceDisabled')
  if (item?.intelligence_status === 'enriched') return t('ipIntelligenceEnriched')
  if (item?.intelligence_status === 'pending' || item?.intelligence_status === 'syncing') return t('ipIntelligencePending')
  if (item?.intelligence_status === 'error') return t('ipIntelligenceError')
  return t('ipIntelligenceNotSynced')
}

async function load() {
  loading.value = true
  errorMessage.value = ''
  try {
    const [nodeResponse, groupResponse, probeResponse, intelligenceStatus] = await Promise.all([
      listNodes({ page: 1, page_size: 1000, compact: true }),
      listNodeGroups(),
      listNodeIpProbes(),
      getNodeIpIntelligenceStatus(),
    ])
    nodes.value = nodeResponse.data
    candidateTotal.value = nodeResponse.pagination?.total ?? nodeResponse.data.length
    groups.value = groupResponse.data
    probes.value = Object.fromEntries(probeResponse.data.map((item) => [item.node_id, item]))
    intelligenceEnabled.value = intelligenceStatus.enabled
    intelligenceBaseUrl.value = intelligenceStatus.base_url
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    loading.value = false
  }
}

function cancelOnLeave() {
  if (!running.value) return
  leavingPage = true
  void cancelNodeIpProbes().catch(() => undefined)
  activeController?.abort()
}

onBeforeRouteLeave(() => cancelOnLeave())
onBeforeUnmount(() => cancelOnLeave())
onMounted(load)
</script>

<template>
  <section class="content-stack ip-probe-page">
    <header class="page-header ip-probe-header">
      <div>
        <span class="eyebrow">{{ t('ipProbeEyebrow') }}</span>
        <h2 class="page-title">{{ t('ipProbeTitle') }}</h2>
        <p class="page-copy">{{ t('ipProbeCopy') }}</p>
      </div>
      <div class="connectivity-run-state" :class="{ active: running }">
        <span class="connectivity-run-light"></span>
        <div>
          <small>MIHOMO EGRESS</small>
          <strong>{{ running ? t('ipProbeRunning') : t('testIdle') }}</strong>
        </div>
      </div>
    </header>

    <div v-if="errorMessage" class="alert alert-error">{{ errorMessage }}</div>
    <div v-if="successMessage" class="alert alert-success">{{ successMessage }}</div>

    <section class="ip-probe-overview">
      <div class="ip-probe-radar" :style="{ '--progress': `${progressPercent * 3.6}deg` }">
        <div class="ip-probe-radar-core">
          <strong>{{ progressPercent }}%</strong>
          <span>{{ provider.toUpperCase() }}</span>
        </div>
      </div>
      <div class="ip-probe-metric"><span>{{ t('selectedNodeCount', { count: selectedIds.length }) }}</span><strong>{{ selectedIds.length }}</strong></div>
      <div class="ip-probe-metric success"><span>{{ t('successfulNodes') }}</span><strong>{{ successfulCount }}</strong></div>
      <div class="ip-probe-metric danger"><span>{{ t('failedNodes') }}</span><strong>{{ failedCount }}</strong></div>
      <div class="ip-probe-metric intelligence"><span>{{ t('ipIntelligenceService') }}</span><strong>{{ intelligenceEnabled ? 'ON' : 'OFF' }}</strong></div>
    </section>

    <section class="card connectivity-node-picker ip-probe-picker">
      <div class="connectivity-picker-head">
        <div>
          <span class="eyebrow">01 / PROBE SET</span>
          <h3>{{ t('selectedNodeCount', { count: selectedIds.length }) }}</h3>
          <p>{{ t('ipProbeLimit') }}</p>
        </div>
        <div class="inline-actions">
          <button class="button button-ghost" :disabled="running || refreshingIntelligence || !intelligenceEnabled || !selectedIds.length" type="button" @click="refreshIntelligence">{{ refreshingIntelligence ? t('refreshing') : t('refreshIpIntelligence') }}</button>
          <button class="button button-ghost" :disabled="running" type="button" @click="selectFiltered">{{ t('selectFiltered') }}</button>
          <button class="button button-ghost" :disabled="running || !selectedIds.length" type="button" @click="clearSelection">{{ t('clearSelection') }}</button>
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
          <input :checked="isSelected(node.id)" :disabled="running" type="checkbox" @change="toggleNode(node.id)" />
          <span class="connectivity-node-check"></span>
          <div><strong>{{ node.name }}</strong><small>{{ node.protocol }} · {{ groupName(node.group_id) }}</small></div>
          <span class="connectivity-node-state">{{ node.enabled ? 'ON' : 'PAUSED' }}</span>
        </label>
      </div>

      <div class="connectivity-action-rail">
        <div><strong>{{ t('ipProbeFixedTarget') }}</strong><span>api64.ipify.org · HTTPS · JSON</span><small v-if="intelligenceEnabled">{{ intelligenceBaseUrl }}</small></div>
        <button v-if="!running" class="button button-accent connectivity-start-button" :disabled="loading || !selectedIds.length" type="button" @click="startProbe">{{ t('startIpProbe') }}</button>
        <button v-else class="button button-danger connectivity-start-button" :disabled="stopping" type="button" @click="stopProbe">{{ stopping ? t('stoppingTest') : t('stopIpProbe') }}</button>
      </div>
    </section>

    <section class="card ip-probe-results">
      <div class="connectivity-picker-head">
        <div><span class="eyebrow">02 / EGRESS MAP</span><h3>{{ t('ipProbeResults') }}</h3><p>{{ t('ipProbeResultsCopy') }}</p></div>
        <span class="connectivity-sample-counter">{{ completed }} / {{ selectedIds.length }}</span>
      </div>
      <div v-if="!selectedNodes.length" class="empty-state">{{ t('selectAtLeastOneNode') }}</div>
      <div v-else class="ip-probe-result-grid">
        <article v-for="node in selectedNodes" :key="node.id" class="ip-probe-result-card" :class="probes[node.id]?.status ?? 'queued'">
          <div class="ip-probe-result-head">
            <span>{{ resultStatus(node.id) }}</span>
            <code>#{{ node.id }}</code>
          </div>
          <strong class="ip-probe-node-name">{{ node.name }}</strong>
          <small>{{ node.protocol }} · {{ groupName(node.group_id) }}</small>
          <div class="ip-probe-address">
            <span>{{ probes[node.id]?.ip_version ? `IPv${probes[node.id]?.ip_version}` : 'IP' }}</span>
            <code>{{ probes[node.id]?.ip ?? '—' }}</code>
          </div>
          <div class="ip-intelligence-result" :class="probes[node.id]?.intelligence_status ?? 'idle'">
            <span>{{ intelligenceLabel(probes[node.id]) }}</span>
            <strong>{{ formatCountryName(probes[node.id]?.country_code) ?? '未识别' }}</strong>
          </div>
          <small v-if="probes[node.id]?.country_source">{{ probes[node.id]?.country_source }}</small>
          <p v-if="probes[node.id]?.intelligence_message" class="connectivity-result-message">{{ probes[node.id]?.intelligence_message }}</p>
          <p v-if="probes[node.id]?.message" class="connectivity-result-message">{{ probes[node.id]?.message }}</p>
          <time v-if="probes[node.id]?.probed_at">{{ probes[node.id]?.probed_at }}</time>
        </article>
      </div>
    </section>
  </section>
</template>
