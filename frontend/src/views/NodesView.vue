<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { extractApiError } from '../api/client'
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
const protocolCount = computed(() => new Set(nodes.value.map((item) => item.protocol)).size)
const compatibilityTargets = computed(() => Object.keys(TARGET_LABELS) as ExportTarget[])
const isEditing = computed(() => editingId.value !== null)
const isEditingGroup = computed(() => editingGroupId.value !== null)
const submitLabel = computed(() => {
  if (saving.value) {
    return isEditing.value ? '保存中...' : '导入中...'
  }
  return isEditing.value ? '保存节点' : '导入节点'
})
const groupSubmitLabel = computed(() => (isEditingGroup.value ? '保存分组' : '创建分组'))

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
    return '测速中'
  }
  const result = currentLatencyResult(item)
  if (!result) {
    return '未测速'
  }
  if (result.status === 'ok') {
    return `${result.latency_ms} ms`
  }
  return result.status === 'timeout' ? '超时' : '失败'
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
    return '未分组'
  }
  return groups.value.find((item) => item.id === groupId)?.name ?? `分组 #${groupId}`
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
      errorMessage.value = `${item.name} 延迟测试失败：${response.data.message ?? response.data.status}`
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
    successMessage.value = `延迟测试完成：${okCount} 个成功，${failedCount} 个失败。`
  } catch (error) {
    errorMessage.value = formatLatencyApiError(error)
  } finally {
    testingLatencyIds.value = testingLatencyIds.value.filter((id) => !ids.includes(id))
  }
}

function formatLatencyApiError(error: unknown) {
  const message = extractApiError(error)
  if (message.includes('Mihomo 内核') || message.toLowerCase().includes('mihomo')) {
    return `${message} 请进入“系统设置”页面检测并下载内核后再测速。`
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
      successMessage.value = '节点已更新。'
    } else {
      const rawLinks = splitRawLinks(form.raw_link)
      if (rawLinks.length === 0) {
        throw new Error('请至少输入一条节点链接。')
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
        throw failures[0]?.item.reason ?? new Error('导入失败。')
      }

      if (failures.length === 0) {
        successMessage.value = successCount === 1 ? '节点已导入。' : `已成功导入 ${successCount} 个节点。`
      } else {
        errorMessage.value = failures
          .slice(0, 3)
          .map((entry) => `${entry.rawLink.slice(0, 42)}: ${extractApiError(entry.item.reason)}`)
          .join('；')
        successMessage.value = `已导入 ${successCount} 个节点，失败 ${failures.length} 个。`
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
    successMessage.value = `已移动 ${selectedNodes.value.length} 个节点到「${groupName(batchGroupId.value)}」。`
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
    const templateMessage = response.template_name ? `，并保存上游模板：${response.template_name}` : ''
    successMessage.value = `已导入 ${response.imported} 个节点，跳过 ${response.skipped} 个重复节点${templateMessage}。`
    if (response.fidelity_warnings.length > 0) {
      const warning = response.fidelity_warnings[0]
      const missing = warning.missing_fields.length > 0 ? `丢失 ${warning.missing_fields.join(', ')}` : ''
      const changed = warning.changed_fields.length > 0 ? `变化 ${warning.changed_fields.join(', ')}` : ''
      errorMessage.value = `转换保真检查发现 ${response.fidelity_warnings.length} 个节点有字段差异。首个：${warning.name} (${warning.protocol}) ${[missing, changed].filter(Boolean).join('；')}`
    }
    if (response.failed > 0) {
      errorMessage.value = `有 ${response.failed} 个节点导入失败：${response.failures[0]?.reason ?? '未知错误'}`
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
      successMessage.value = '节点分组已更新。'
    } else {
      await createNodeGroup(payload)
      successMessage.value = '节点分组已创建。'
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
  if (!window.confirm('确定删除这个节点吗？')) {
    return
  }

  try {
    await deleteNode(id)
    if (editingId.value === id) {
      closeEditor()
    }
    selectedIds.value = selectedIds.value.filter((item) => item !== id)
    successMessage.value = '节点已删除。'
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
  if (!window.confirm(`确定删除已选择的 ${count} 个节点吗？此操作不可恢复。`)) {
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
      throw failures[0]?.reason ?? new Error('批量删除失败。')
    }

    if (editingId.value !== null && ids.includes(editingId.value)) {
      closeEditor()
    }

    selectedIds.value = []
    successMessage.value = `已删除 ${successCount} 个节点。`
    if (failures.length > 0) {
      errorMessage.value = `有 ${failures.length} 个节点删除失败：${extractApiError(failures[0].reason)}`
    }
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    saving.value = false
  }
}

async function removeGroup(id: number) {
  if (!window.confirm('确定删除这个分组吗？')) {
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
    successMessage.value = '节点分组已删除。'
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
        <h2 class="page-title">节点管理</h2>
        <p class="page-copy">共 {{ nodes.length }} 个节点，覆盖 {{ protocolCount }} 种协议，{{ groups.length }} 个分组。</p>
      </div>
      <div class="inline-actions">
        <select v-model="groupFilter" class="select toolbar-select" aria-label="节点分组筛选">
          <option value="all">全部分组</option>
          <option value="none">未分组</option>
          <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option>
        </select>
        <button class="button button-ghost" type="button" :disabled="loading" @click="load">
          {{ loading ? '刷新中...' : '刷新' }}
        </button>
        <button class="button button-ghost" type="button" @click="openGroupEditor">分组管理</button>
        <button class="button button-ghost" type="button" @click="openUpstreamImporter">上游订阅导入</button>
        <button class="button button-accent" type="button" @click="openCreate">导入节点</button>
      </div>
    </header>

    <div v-if="errorMessage" class="error-banner">{{ errorMessage }}</div>
    <div v-if="successMessage" class="success-banner">{{ successMessage }}</div>

    <article class="card stack management-card">
      <div class="section-bar">
        <div>
          <div class="hint">节点列表</div>
          <p class="card-copy">当前筛选 {{ filteredNodes.length }} 个，已选择 {{ selectedCount }} 个。</p>
        </div>
        <div class="inline-actions bulk-actions">
          <select v-model="batchGroupId" class="select toolbar-select" aria-label="批量移动到分组">
            <option :value="null">移动到未分组</option>
            <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option>
          </select>
          <button class="button button-ghost" type="button" :disabled="selectedCount === 0" @click="clearSelection">清空选择</button>
          <button class="button button-accent" type="button" :disabled="saving || selectedCount === 0" @click="moveSelectedNodes">
            移动分组
          </button>
          <button class="button button-ghost" type="button" :disabled="selectedCount === 0" @click="testSelectedLatencies">
            测试延迟
          </button>
          <button class="button button-danger" type="button" :disabled="saving || selectedCount === 0" @click="removeSelectedNodes">
            删除所选
          </button>
        </div>
      </div>

      <div v-if="filteredNodes.length === 0" class="empty-state">这个分组下还没有节点。</div>

      <div v-else class="table-wrap">
        <table class="table dense-table selectable-table node-table">
          <thead>
            <tr>
              <th class="selection-cell">
                <input
                  type="checkbox"
                  :checked="currentPageSelected"
                  :aria-label="currentPageSelected ? '取消全选当前页' : '全选当前页'"
                  @change="toggleCurrentPage(($event.target as HTMLInputElement).checked)"
                />
              </th>
              <th>节点</th>
              <th>连接</th>
              <th>兼容</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="item in pagedNodes" :key="item.id" :class="nodeRowClass(item)">
              <td class="selection-cell">
                <input v-model="selectedIds" :value="item.id" type="checkbox" :aria-label="`选择 ${item.name}`" />
              </td>
              <td class="node-name-cell">
                <div class="subscription-title-row">
                  <strong class="row-title">{{ item.name }}</strong>
                  <span class="status-badge status-badge-neutral">{{ item.enabled ? '启用' : '停用' }}</span>
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
                  {{ supportedTargets(item).length }}/{{ compatibilityTargets.length }} supported
                </div>
              </td>
              <td>
                <div class="inline-actions row-actions">
                  <button class="button button-ghost button-compact" type="button" :disabled="isTestingLatency(item.id)" @click="testLatency(item)">
                    {{ isTestingLatency(item.id) ? '测速中' : '测速' }}
                  </button>
                  <button class="button button-ghost button-compact" type="button" @click="startEdit(item)">编辑</button>
                  <button class="button button-danger button-compact" type="button" @click="removeNode(item.id)">删除</button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <footer class="pagination-bar">
        <span class="hint">第 {{ page }} / {{ pageCount }} 页</span>
        <select v-model.number="pageSize" class="select page-size-select" aria-label="每页数量">
          <option v-for="size in PAGE_SIZE_OPTIONS" :key="size" :value="size">每页 {{ size }}</option>
        </select>
        <div class="inline-actions">
          <button class="button button-ghost button-compact" type="button" :disabled="page <= 1" @click="page -= 1">上一页</button>
          <button class="button button-ghost button-compact" type="button" :disabled="page >= pageCount" @click="page += 1">下一页</button>
        </div>
      </footer>
    </article>

    <Teleport to="body">
      <div v-if="showEditor" class="modal-backdrop" @click.self="closeEditor">
        <section class="modal-panel">
          <header class="modal-header">
            <div>
              <span class="eyebrow">{{ isEditing ? 'Edit Node' : 'Import Nodes' }}</span>
              <h3>{{ isEditing ? '编辑节点' : '导入节点' }}</h3>
            </div>
            <button class="icon-button" type="button" aria-label="关闭" @click="closeEditor">×</button>
          </header>

          <form class="form-grid" @submit.prevent="submit">
            <div>
              <label class="field-label" for="node-name">显示名称</label>
              <input id="node-name" v-model.trim="form.name" class="input" placeholder="可选，批量导入时自动使用链接名称" />
            </div>

            <div>
              <label class="field-label" for="node-group">节点分组</label>
              <select id="node-group" v-model="form.group_id" class="select">
                <option :value="null">未分组</option>
                <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option>
              </select>
            </div>

            <div>
              <label class="field-label" for="node-link">原始链接</label>
              <textarea id="node-link" v-model.trim="form.raw_link" class="textarea code-textarea" placeholder="一行一条节点链接" />
              <div class="hint template-kind-hint">新增支持多行批量导入；编辑模式按单条节点处理。</div>
            </div>

            <div>
              <label class="field-label" for="node-remark">备注</label>
              <input id="node-remark" v-model.trim="form.remark" class="input" placeholder="可选" />
            </div>

            <label v-if="isEditing" class="checkbox-item">
              <input v-model="form.enabled" type="checkbox" />
              <span>
                <strong>启用节点</strong>
                <div class="muted">关闭后保留记录，但不会作为可用节点对外分发。</div>
              </span>
            </label>

            <div class="modal-actions">
              <button class="button button-ghost" type="button" :disabled="saving" @click="closeEditor">取消</button>
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
              <h3>从上游订阅导入</h3>
            </div>
            <button class="icon-button" type="button" aria-label="关闭" @click="closeUpstreamImporter">x</button>
          </header>

          <form class="form-grid" @submit.prevent="importFromUpstreamSubscription">
            <div>
              <label class="field-label" for="upstream-url">上游订阅链接</label>
              <input
                id="upstream-url"
                v-model.trim="upstreamForm.url"
                class="input"
                placeholder="https://example.com/sub..."
              />
              <div class="hint template-kind-hint">支持 URI/Base64 订阅，也支持 Mihomo YAML 的 proxies 列表。</div>
            </div>

            <div>
              <label class="field-label" for="upstream-group">导入到节点分组</label>
              <select id="upstream-group" v-model="upstreamForm.group_id" class="select">
                <option :value="null">未分组</option>
                <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option>
              </select>
            </div>

            <div>
              <label class="field-label" for="upstream-remark">备注</label>
              <input id="upstream-remark" v-model.trim="upstreamForm.remark" class="input" placeholder="可选，例如：机场 A" />
            </div>

            <div class="modal-actions">
              <button class="button button-ghost" type="button" :disabled="saving" @click="closeUpstreamImporter">取消</button>
              <button class="button button-accent" type="submit" :disabled="saving || !upstreamForm.url">
                {{ saving ? '导入中...' : '开始导入' }}
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
              <h3>节点分组</h3>
            </div>
            <button class="icon-button" type="button" aria-label="关闭" @click="closeGroupEditor">×</button>
          </header>

          <form class="form-grid" @submit.prevent="submitGroup">
            <div>
              <label class="field-label" for="node-group-name">分组名称</label>
              <input id="node-group-name" v-model.trim="groupForm.name" class="input" placeholder="例如：香港线路" />
            </div>
            <div>
              <label class="field-label" for="node-group-order">排序</label>
              <input id="node-group-order" v-model.number="groupForm.sort_order" class="input" type="number" />
            </div>
            <div class="modal-actions">
              <button class="button button-ghost" type="button" @click="resetGroupForm">清空</button>
              <button class="button button-accent" type="submit" :disabled="saving || !groupForm.name">{{ groupSubmitLabel }}</button>
            </div>
          </form>

          <div class="group-list">
            <div v-for="group in groups" :key="group.id" class="group-row">
              <span>{{ group.name }}</span>
              <span class="muted">sort {{ group.sort_order }}</span>
              <div class="inline-actions">
                <button class="button button-ghost button-compact" type="button" @click="startEditGroup(group)">编辑</button>
                <button class="button button-danger button-compact" type="button" @click="removeGroup(group.id)">删除</button>
              </div>
            </div>
          </div>
        </section>
      </div>
    </Teleport>
  </section>
</template>
