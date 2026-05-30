<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { extractApiError } from '../api/client'
import { useI18n } from '../i18n'
import {
  createNode,
  deleteNode,
  importNodesFromSubscription,
  listNodes,
  testNodeLatency,
  testNodeLatencyBatch,
  updateNode,
  type NodeLatencyResult,
  type NodeItem,
} from '../api/nodes'
import {
  createNodeGroup,
  deleteNodeGroup,
  listNodeGroups,
  updateNodeGroup,
  type GroupItem,
} from '../api/groups'

const { t } = useI18n()

type ExportTarget = 'mihomo' | 'xray' | 'surge' | 'sing-box'

const TARGET_LABELS: Record<ExportTarget, string> = {
  mihomo: 'Mihomo',
  xray: 'Xray',
  surge: 'Surge',
  'sing-box': 'sing-box',
}

const TARGET_SUPPORT: Record<ExportTarget, Set<string>> = {
  mihomo: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2', 'tuic', 'wireguard', 'anytls']),
  xray: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2']),
  surge: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2', 'tuic', 'wireguard']),
  'sing-box': new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2', 'tuic', 'wireguard', 'anytls']),
}

const PAGE_SIZE_OPTIONS = [10, 20, 50, 100]

const nodes = ref<NodeItem[]>([])
const groups = ref<GroupItem[]>([])
const loading = ref(false)
const saving = ref(false)
const showEditor = ref(false)
const showUpstreamImporter = ref(false)
const showGroupEditor = ref(false)
const editingId = ref<number | null>(null)
const editingGroupId = ref<number | null>(null)
const groupFilter = ref<number | 'all' | 'none'>('all')
const selectedIds = ref<number[]>([])
const batchGroupId = ref<number | null>(null)
const page = ref(1)
const pageSize = ref(20)
const errorMessage = ref('')
const successMessage = ref('')
const latencyResults = ref<Record<number, NodeLatencyResult>>({})
const testingLatencyIds = ref<number[]>([])

const form = reactive({
  name: '',
  raw_link: '',
  remark: '',
  group_id: null as number | null,
  enabled: true,
})

const upstreamForm = reactive({
  url: '',
  group_id: null as number | null,
  remark: '',
})

const groupForm = reactive({
  name: '',
  sort_order: 0,
})

const filteredNodes = computed(() => {
  if (groupFilter.value === 'all') {
    return nodes.value
  }
  if (groupFilter.value === 'none') {
    return nodes.value.filter((item) => item.group_id === null)
  }
  return nodes.value.filter((item) => item.group_id === groupFilter.value)
})

const pageCount = computed(() => Math.max(1, Math.ceil(filteredNodes.value.length / pageSize.value)))
const pagedNodes = computed(() => {
  const start = (page.value - 1) * pageSize.value
  return filteredNodes.value.slice(start, start + pageSize.value)
})
const selectedNodes = computed(() => nodes.value.filter((item) => selectedIds.value.includes(item.id)))
const currentPageIds = computed(() => pagedNodes.value.map((item) => item.id))
const currentPageSelected = computed(
  () => currentPageIds.value.length > 0 && currentPageIds.value.every((id) => selectedIds.value.includes(id)),
)
const selectedCount = computed(() => selectedIds.value.length)
const compatibilityTargets = computed(() => Object.keys(TARGET_LABELS) as ExportTarget[])
const isEditing = computed(() => editingId.value !== null)
const isEditingGroup = computed(() => editingGroupId.value !== null)
const submitLabel = computed(() => {
  if (saving.value) {
    return isEditing.value ? t('savingNode') : t('importingNode')
  }
  return isEditing.value ? t('saveNode') : t('importNode')
})
const groupSubmitLabel = computed(() => (isEditingGroup.value ? t('saveGroup') : t('createGroup')))

watch([filteredNodes, pageSize], () => {
  page.value = Math.min(page.value, pageCount.value)
  selectedIds.value = selectedIds.value.filter((id) => filteredNodes.value.some((item) => item.id === id))
})

watch(groupFilter, () => {
  page.value = 1
  selectedIds.value = []
})

function splitRawLinks(rawText: string) {
  return rawText
    .split(/\r?\n/)
    .map((item) => item.trim())
    .filter((item) => item.length > 0)
}

function isNodeSupportedForTarget(node: Pick<NodeItem, 'protocol'>, target: ExportTarget) {
  return TARGET_SUPPORT[target].has(node.protocol)
}

function supportedTargets(node: Pick<NodeItem, 'protocol'>) {
  return compatibilityTargets.value.filter((target) => isNodeSupportedForTarget(node, target))
}

function isTestingLatency(id: number) {
  return testingLatencyIds.value.includes(id)
}

function setLatencyResult(result: NodeLatencyResult) {
  latencyResults.value = {
    ...latencyResults.value,
    [result.id]: result,
  }
}

function savedLatencyResult(item: NodeItem): NodeLatencyResult | null {
  if (!item.last_latency_status || !item.last_latency_tested_at) {
    return null
  }
  return {
    id: item.id,
    status: item.last_latency_status,
    latency_ms: item.last_latency_ms,
    message: item.last_latency_message,
    tested_at: item.last_latency_tested_at,
  }
}

function currentLatencyResult(item: NodeItem) {
  return latencyResults.value[item.id] ?? savedLatencyResult(item)
}

function latencyText(item: NodeItem) {
  if (isTestingLatency(item.id)) {
    return t('testing')
  }
  const result = currentLatencyResult(item)
  if (!result) {
    return t('untested')
  }
  if (result.status === 'ok') {
    return `${result.latency_ms} ms`
  }
  return result.status === 'timeout' ? t('timeout') : t('failed')
}

function latencyClass(item: NodeItem) {
  const result = currentLatencyResult(item)
  if (isTestingLatency(item.id)) {
    return 'metric-chip'
  }
  if (!result) {
    return 'metric-chip'
  }
  if (result.status !== 'ok') {
    return 'metric-chip metric-chip-warn'
  }
  const value = result.latency_ms ?? 9999
  if (value <= 200) {
    return 'metric-chip metric-chip-ok'
  }
  if (value <= 800) {
    return 'metric-chip'
  }
  return 'metric-chip metric-chip-warn'
}

function nodeRowClass(item: NodeItem) {
  return {
    'row-selected': selectedIds.value.includes(item.id),
    'row-latency-bad': currentLatencyResult(item)?.status && currentLatencyResult(item)?.status !== 'ok',
  }
}

function groupName(groupId: number | null) {
  if (groupId === null) {
    return t('ungrouped')
  }
  return groups.value.find((item) => item.id === groupId)?.name ?? t('groupFallback', { id: groupId })
}

function resetForm() {
  editingId.value = null
  form.name = ''
  form.raw_link = ''
  form.remark = ''
  form.group_id = null
  form.enabled = true
}

function resetGroupForm() {
  editingGroupId.value = null
  groupForm.name = ''
  groupForm.sort_order = 0
}

function openCreate() {
  resetForm()
  showEditor.value = true
  errorMessage.value = ''
  successMessage.value = ''
}

function closeEditor() {
  showEditor.value = false
  resetForm()
}

function openUpstreamImporter() {
  upstreamForm.url = ''
  upstreamForm.group_id = null
  upstreamForm.remark = ''
  showUpstreamImporter.value = true
  errorMessage.value = ''
  successMessage.value = ''
}

function closeUpstreamImporter() {
  showUpstreamImporter.value = false
  upstreamForm.url = ''
  upstreamForm.group_id = null
  upstreamForm.remark = ''
}

function openGroupEditor() {
  resetGroupForm()
  showGroupEditor.value = true
  errorMessage.value = ''
  successMessage.value = ''
}

function closeGroupEditor() {
  showGroupEditor.value = false
  resetGroupForm()
}

function startEdit(item: NodeItem) {
  editingId.value = item.id
  form.name = item.name
  form.raw_link = item.raw_link
  form.remark = item.remark
  form.group_id = item.group_id
  form.enabled = item.enabled
  showEditor.value = true
  errorMessage.value = ''
  successMessage.value = ''
}

function startEditGroup(item: GroupItem) {
  editingGroupId.value = item.id
  groupForm.name = item.name
  groupForm.sort_order = item.sort_order
}

function toggleCurrentPage(checked: boolean) {
  if (checked) {
    selectedIds.value = Array.from(new Set([...selectedIds.value, ...currentPageIds.value]))
    return
  }
  selectedIds.value = selectedIds.value.filter((id) => !currentPageIds.value.includes(id))
}

function clearSelection() {
  selectedIds.value = []
}

async function testLatency(item: NodeItem) {
  if (isTestingLatency(item.id)) {
    return
  }
  testingLatencyIds.value = [...testingLatencyIds.value, item.id]
  errorMessage.value = ''

  try {
    const response = await testNodeLatency(item.id)
    setLatencyResult(response.data)
    if (response.data.status !== 'ok') {
      errorMessage.value = t('latencyFailed', { name: item.name, message: response.data.message ?? response.data.status })
    }
  } catch (error) {
    errorMessage.value = formatLatencyApiError(error)
  } finally {
    testingLatencyIds.value = testingLatencyIds.value.filter((id) => id !== item.id)
  }
}

async function testSelectedLatencies() {
  if (selectedIds.value.length === 0) {
    return
  }
  const ids = [...selectedIds.value]
  testingLatencyIds.value = Array.from(new Set([...testingLatencyIds.value, ...ids]))
  errorMessage.value = ''
  successMessage.value = ''

  try {
    const response = await testNodeLatencyBatch(ids)
    for (const result of response.data) {
      setLatencyResult(result)
    }
    const okCount = response.data.filter((item) => item.status === 'ok').length
    const failedCount = response.data.length - okCount
    successMessage.value = t('latencyBatchDone', { ok: okCount, failed: failedCount })
  } catch (error) {
    errorMessage.value = formatLatencyApiError(error)
  } finally {
    testingLatencyIds.value = testingLatencyIds.value.filter((id) => !ids.includes(id))
  }
}

function formatLatencyApiError(error: unknown) {
  const message = extractApiError(error)
  const mihomoCoreText = 'Mihomo \u5185\u6838'
  if (message.includes(mihomoCoreText) || message.toLowerCase().includes('mihomo')) {
    return `${message} ${t('latencyCoreRequired')}`
  }
  return message
}

async function load() {
  loading.value = true
  errorMessage.value = ''

  try {
    const [nodeResponse, groupResponse] = await Promise.all([listNodes(), listNodeGroups()])
    nodes.value = nodeResponse.data
    groups.value = groupResponse.data
    page.value = Math.min(page.value, pageCount.value)
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
    if (editingId.value !== null) {
      await updateNode(editingId.value, {
        name: form.name || undefined,
        raw_link: form.raw_link,
        group_id: form.group_id,
        remark: form.remark || undefined,
        enabled: form.enabled,
      })
      successMessage.value = t('nodeUpdated')
    } else {
      const rawLinks = splitRawLinks(form.raw_link)
      if (rawLinks.length === 0) {
        throw new Error(t('nodeLinkRequired'))
      }

      const results = await Promise.allSettled(
        rawLinks.map((rawLink) =>
          createNode({
            name: rawLinks.length === 1 ? form.name || undefined : undefined,
            raw_link: rawLink,
            group_id: form.group_id,
            remark: form.remark || undefined,
          }),
        ),
      )
      const successCount = results.filter((item) => item.status === 'fulfilled').length
      const failures = results
        .map((item, index) => ({ item, rawLink: rawLinks[index] }))
        .filter((entry): entry is { item: PromiseRejectedResult; rawLink: string } => entry.item.status === 'rejected')

      if (successCount === 0) {
        throw failures[0]?.item.reason ?? new Error(t('importFailed'))
      }

      if (failures.length === 0) {
        successMessage.value = successCount === 1 ? t('nodeImported') : t('nodesImported', { count: successCount })
      } else {
        errorMessage.value = failures
          .slice(0, 3)
          .map((entry) => `${entry.rawLink.slice(0, 42)}: ${extractApiError(entry.item.reason)}`)
          .join('；')
        successMessage.value = t('nodesImportPartial', { success: successCount, failed: failures.length })
      }
    }

    closeEditor()
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    saving.value = false
  }
}

async function moveSelectedNodes() {
  if (selectedNodes.value.length === 0) {
    return
  }

  saving.value = true
  errorMessage.value = ''
  successMessage.value = ''

  try {
    await Promise.all(
      selectedNodes.value.map((item) =>
        updateNode(item.id, {
          name: item.name,
          raw_link: item.raw_link,
          group_id: batchGroupId.value,
          remark: item.remark || undefined,
          enabled: item.enabled,
        }),
      ),
    )
    successMessage.value = t('nodesMoved', { count: selectedNodes.value.length, group: groupName(batchGroupId.value) })
    clearSelection()
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    saving.value = false
  }
}

async function importFromUpstreamSubscription() {
  saving.value = true
  errorMessage.value = ''
  successMessage.value = ''

  try {
    const response = await importNodesFromSubscription({
      url: upstreamForm.url,
      group_id: upstreamForm.group_id,
      remark: upstreamForm.remark || undefined,
    })
    const templateMessage = response.template_name ? t('upstreamTemplateSaved', { name: response.template_name }) : ''
    successMessage.value = t('upstreamImported', { imported: response.imported, skipped: response.skipped, template: templateMessage })
    if (response.fidelity_warnings.length > 0) {
      const warning = response.fidelity_warnings[0]
      const missing = warning.missing_fields.length > 0 ? t('fidelityMissing', { fields: warning.missing_fields.join(', ') }) : ''
      const changed = warning.changed_fields.length > 0 ? t('fidelityChanged', { fields: warning.changed_fields.join(', ') }) : ''
      errorMessage.value = t('fidelityWarning', {
        count: response.fidelity_warnings.length,
        name: warning.name,
        protocol: warning.protocol,
        detail: [missing, changed].filter(Boolean).join('；'),
      })
    }
    if (response.failed > 0) {
      errorMessage.value = t('nodeImportFailures', { count: response.failed, reason: response.failures[0]?.reason ?? t('unknown') })
    }
    closeUpstreamImporter()
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    saving.value = false
  }
}

async function submitGroup() {
  saving.value = true
  errorMessage.value = ''
  successMessage.value = ''

  try {
    const payload = { name: groupForm.name, sort_order: groupForm.sort_order }
    if (editingGroupId.value !== null) {
      await updateNodeGroup(editingGroupId.value, payload)
      successMessage.value = t('nodeGroupUpdated')
    } else {
      await createNodeGroup(payload)
      successMessage.value = t('nodeGroupCreated')
    }
    resetGroupForm()
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    saving.value = false
  }
}

async function removeNode(id: number) {
  if (!window.confirm(t('confirmDeleteNode'))) {
    return
  }

  try {
    await deleteNode(id)
    if (editingId.value === id) {
      closeEditor()
    }
    selectedIds.value = selectedIds.value.filter((item) => item !== id)
    successMessage.value = t('nodeDeleted')
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  }
}

async function removeSelectedNodes() {
  if (selectedNodes.value.length === 0) {
    return
  }

  const count = selectedNodes.value.length
  if (!window.confirm(t('confirmDeleteSelectedNodes', { count }))) {
    return
  }

  saving.value = true
  errorMessage.value = ''
  successMessage.value = ''

  try {
    const ids = selectedNodes.value.map((item) => item.id)
    const results = await Promise.allSettled(ids.map((id) => deleteNode(id)))
    const successCount = results.filter((item) => item.status === 'fulfilled').length
    const failures = results.filter((item): item is PromiseRejectedResult => item.status === 'rejected')

    if (successCount === 0) {
      throw failures[0]?.reason ?? new Error(t('batchDeleteFailed'))
    }

    if (editingId.value !== null && ids.includes(editingId.value)) {
      closeEditor()
    }

    selectedIds.value = []
    successMessage.value = t('nodesDeleted', { count: successCount })
    if (failures.length > 0) {
      errorMessage.value = t('nodeDeleteFailures', { count: failures.length, reason: extractApiError(failures[0].reason) })
    }
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    saving.value = false
  }
}

async function removeGroup(id: number) {
  if (!window.confirm(t('confirmDeleteGroup'))) {
    return
  }

  try {
    await deleteNodeGroup(id)
    if (groupFilter.value === id) {
      groupFilter.value = 'all'
    }
    if (batchGroupId.value === id) {
      batchGroupId.value = null
    }
    successMessage.value = t('nodeGroupDeleted')
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  }
}

onMounted(load)
</script>

<template>
  <section class="stack">
    <header class="page-header">
      <div>
        <span class="eyebrow">Nodes</span>
        <h2 class="page-title">{{ t('nodes') }}</h2>
        <p class="page-copy">{{ t('nodesCopy') }}</p>
      </div>
      <div class="inline-actions">
        <select v-model="groupFilter" class="select toolbar-select" :aria-label="t('nodeGroup')">
          <option value="all">{{ t('allGroups') }}</option>
          <option value="none">{{ t('ungrouped') }}</option>
          <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option>
        </select>
        <button class="button button-ghost" type="button" :disabled="loading" @click="load">
          {{ loading ? t('refreshing') : t('refresh') }}
        </button>
        <button class="button button-ghost" type="button" @click="openGroupEditor">{{ t('groupManagement') }}</button>
        <button class="button button-ghost" type="button" @click="openUpstreamImporter">{{ t('upstreamImport') }}</button>
        <button class="button button-accent" type="button" @click="openCreate">{{ t('importNode') }}</button>
      </div>
    </header>

    <div v-if="errorMessage" class="error-banner">{{ errorMessage }}</div>
    <div v-if="successMessage" class="success-banner">{{ successMessage }}</div>

    <article class="card stack management-card">
      <div class="section-bar">
        <div>
          <div class="hint">{{ t('nodeList') }}</div>
          <p class="card-copy">{{ t('nodeListSummary', { filtered: filteredNodes.length, selected: selectedCount }) }}</p>
        </div>
        <div class="inline-actions bulk-actions">
          <select v-model="batchGroupId" class="select toolbar-select" :aria-label="t('moveGroup')">
            <option :value="null">{{ t('moveToUngrouped') }}</option>
            <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option>
          </select>
          <button class="button button-ghost" type="button" :disabled="selectedCount === 0" @click="clearSelection">{{ t('clearSelection') }}</button>
          <button class="button button-accent" type="button" :disabled="saving || selectedCount === 0" @click="moveSelectedNodes">
            {{ t('moveGroup') }}
          </button>
          <button class="button button-ghost" type="button" :disabled="selectedCount === 0" @click="testSelectedLatencies">
            {{ t('testLatency') }}
          </button>
          <button class="button button-danger" type="button" :disabled="saving || selectedCount === 0" @click="removeSelectedNodes">
            {{ t('deleteSelected') }}
          </button>
        </div>
      </div>

      <div v-if="filteredNodes.length === 0" class="empty-state">{{ t('emptyGroupNodes') }}</div>

      <div v-else class="table-wrap">
        <table class="table dense-table selectable-table node-table">
          <thead>
            <tr>
              <th class="selection-cell">
                <input
                  type="checkbox"
                  :checked="currentPageSelected"
                  :aria-label="currentPageSelected ? t('unselectCurrentPage') : t('selectCurrentPage')"
                  @change="toggleCurrentPage(($event.target as HTMLInputElement).checked)"
                />
              </th>
              <th>{{ t('node') }}</th>
              <th>{{ t('connection') }}</th>
              <th>{{ t('compatibility') }}</th>
              <th>{{ t('actions') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="item in pagedNodes" :key="item.id" :class="nodeRowClass(item)">
              <td class="selection-cell">
                <input v-model="selectedIds" :value="item.id" type="checkbox" :aria-label="t('selectNode', { name: item.name })" />
              </td>
              <td class="node-name-cell">
                <div class="subscription-title-row">
                  <strong class="row-title">{{ item.name }}</strong>
                  <span class="status-badge status-badge-neutral">{{ item.enabled ? t('enabled') : t('disabled') }}</span>
                </div>
                <div class="row-meta">
                  #{{ item.id }} · {{ groupName(item.group_id) }}
                </div>
                <div v-if="item.remark" class="row-note">{{ item.remark }}</div>
              </td>
              <td>
                <span class="metric-chip">{{ item.protocol }}</span>
                <span :class="latencyClass(item)" :title="currentLatencyResult(item)?.message ?? ''">
                  {{ latencyText(item) }}
                </span>
                <div class="row-meta">{{ item.server }}:{{ item.port }}</div>
              </td>
              <td>
                <div class="target-badge-grid">
                  <span
                    v-for="target in compatibilityTargets"
                    :key="target"
                    class="status-badge"
                    :class="isNodeSupportedForTarget(item, target) ? 'status-badge-ok' : 'status-badge-warn'"
                  >
                    {{ TARGET_LABELS[target] }}
                  </span>
                </div>
                <div class="muted compat-summary">
                  {{ supportedTargets(item).length }}/{{ compatibilityTargets.length }} {{ t('supported') }}
                </div>
              </td>
              <td>
                <div class="inline-actions row-actions">
                  <button class="button button-ghost button-compact" type="button" :disabled="isTestingLatency(item.id)" @click="testLatency(item)">
                    {{ isTestingLatency(item.id) ? t('testing') : t('test') }}
                  </button>
                  <button class="button button-ghost button-compact" type="button" @click="startEdit(item)">{{ t('edit') }}</button>
                  <button class="button button-danger button-compact" type="button" @click="removeNode(item.id)">{{ t('delete') }}</button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <footer class="pagination-bar">
        <span class="hint">{{ t('pageLabel', { page, count: pageCount }) }}</span>
        <select v-model.number="pageSize" class="select page-size-select" :aria-label="t('pageSize', { size: pageSize })">
          <option v-for="size in PAGE_SIZE_OPTIONS" :key="size" :value="size">{{ t('pageSize', { size }) }}</option>
        </select>
        <div class="inline-actions">
          <button class="button button-ghost button-compact" type="button" :disabled="page <= 1" @click="page -= 1">{{ t('previousPage') }}</button>
          <button class="button button-ghost button-compact" type="button" :disabled="page >= pageCount" @click="page += 1">{{ t('nextPage') }}</button>
        </div>
      </footer>
    </article>

    <Teleport to="body">
      <div v-if="showEditor" class="modal-backdrop" @click.self="closeEditor">
        <section class="modal-panel">
          <header class="modal-header">
            <div>
              <span class="eyebrow">{{ isEditing ? 'Edit Node' : 'Import Nodes' }}</span>
              <h3>{{ isEditing ? t('editNode') : t('importNodes') }}</h3>
            </div>
            <button class="icon-button" type="button" :aria-label="t('close')" @click="closeEditor">×</button>
          </header>

          <form class="form-grid" @submit.prevent="submit">
            <div>
              <label class="field-label" for="node-name">{{ t('displayName') }}</label>
              <input id="node-name" v-model.trim="form.name" class="input" :placeholder="t('nodeNamePlaceholder')" />
            </div>

            <div>
              <label class="field-label" for="node-group">{{ t('nodeGroup') }}</label>
              <select id="node-group" v-model="form.group_id" class="select">
                <option :value="null">{{ t('ungrouped') }}</option>
                <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option>
              </select>
            </div>

            <div>
              <label class="field-label" for="node-link">{{ t('rawLink') }}</label>
              <textarea id="node-link" v-model.trim="form.raw_link" class="textarea code-textarea" :placeholder="t('rawLinkPlaceholder')" />
              <div class="hint template-kind-hint">{{ t('rawLinkHint') }}</div>
            </div>

            <div>
              <label class="field-label" for="node-remark">{{ t('remark') }}</label>
              <input id="node-remark" v-model.trim="form.remark" class="input" :placeholder="t('optional')" />
            </div>

            <label v-if="isEditing" class="checkbox-item">
              <input v-model="form.enabled" type="checkbox" />
              <span>
                <strong>{{ t('enableNode') }}</strong>
                <div class="muted">{{ t('enableNodeHint') }}</div>
              </span>
            </label>

            <div class="modal-actions">
              <button class="button button-ghost" type="button" :disabled="saving" @click="closeEditor">{{ t('cancel') }}</button>
              <button class="button button-accent" type="submit" :disabled="saving || !form.raw_link">{{ submitLabel }}</button>
            </div>
          </form>
        </section>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-if="showUpstreamImporter" class="modal-backdrop" @click.self="closeUpstreamImporter">
        <section class="modal-panel">
          <header class="modal-header">
            <div>
              <span class="eyebrow">Upstream</span>
              <h3>{{ t('upstreamImportTitle') }}</h3>
            </div>
            <button class="icon-button" type="button" :aria-label="t('close')" @click="closeUpstreamImporter">x</button>
          </header>

          <form class="form-grid" @submit.prevent="importFromUpstreamSubscription">
            <div>
              <label class="field-label" for="upstream-url">{{ t('upstreamUrl') }}</label>
              <input
                id="upstream-url"
                v-model.trim="upstreamForm.url"
                class="input"
                placeholder="https://example.com/sub..."
              />
              <div class="hint template-kind-hint">{{ t('upstreamHint') }}</div>
            </div>

            <div>
              <label class="field-label" for="upstream-group">{{ t('importToNodeGroup') }}</label>
              <select id="upstream-group" v-model="upstreamForm.group_id" class="select">
                <option :value="null">{{ t('ungrouped') }}</option>
                <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option>
              </select>
            </div>

            <div>
              <label class="field-label" for="upstream-remark">{{ t('remark') }}</label>
              <input id="upstream-remark" v-model.trim="upstreamForm.remark" class="input" :placeholder="t('remarkPlaceholder')" />
            </div>

            <div class="modal-actions">
              <button class="button button-ghost" type="button" :disabled="saving" @click="closeUpstreamImporter">{{ t('cancel') }}</button>
              <button class="button button-accent" type="submit" :disabled="saving || !upstreamForm.url">
                {{ saving ? t('importingNode') : t('startImport') }}
              </button>
            </div>
          </form>
        </section>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-if="showGroupEditor" class="modal-backdrop" @click.self="closeGroupEditor">
        <section class="modal-panel">
          <header class="modal-header">
            <div>
              <span class="eyebrow">Node Groups</span>
              <h3>{{ t('nodeGroups') }}</h3>
            </div>
            <button class="icon-button" type="button" :aria-label="t('close')" @click="closeGroupEditor">×</button>
          </header>

          <form class="form-grid" @submit.prevent="submitGroup">
            <div>
              <label class="field-label" for="node-group-name">{{ t('groupName') }}</label>
              <input id="node-group-name" v-model.trim="groupForm.name" class="input" :placeholder="t('groupNamePlaceholder')" />
            </div>
            <div>
              <label class="field-label" for="node-group-order">{{ t('sortOrder') }}</label>
              <input id="node-group-order" v-model.number="groupForm.sort_order" class="input" type="number" />
            </div>
            <div class="modal-actions">
              <button class="button button-ghost" type="button" @click="resetGroupForm">{{ t('clear') }}</button>
              <button class="button button-accent" type="submit" :disabled="saving || !groupForm.name">{{ groupSubmitLabel }}</button>
            </div>
          </form>

          <div class="group-list">
            <div v-for="group in groups" :key="group.id" class="group-row">
              <span>{{ group.name }}</span>
              <span class="muted">sort {{ group.sort_order }}</span>
              <div class="inline-actions">
                <button class="button button-ghost button-compact" type="button" @click="startEditGroup(group)">{{ t('edit') }}</button>
                <button class="button button-danger button-compact" type="button" @click="removeGroup(group.id)">{{ t('delete') }}</button>
              </div>
            </div>
          </div>
        </section>
      </div>
    </Teleport>
  </section>
</template>
