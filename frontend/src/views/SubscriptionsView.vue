<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { extractApiError } from '../api/client'
import { listNodes, type NodeItem } from '../api/nodes'
import { listTemplates, type TemplateItem } from '../api/templates'
import {
  createSubscription,
  deleteSubscription,
  listSubscriptions,
  renewSubscription,
  rotateSubscriptionToken,
  updateSubscription,
  type SubscriptionItem,
} from '../api/subscriptions'
import {
  createSubscriptionGroup,
  deleteSubscriptionGroup,
  listNodeGroups,
  listSubscriptionGroups,
  updateSubscriptionGroup,
  type GroupItem,
} from '../api/groups'

type ExportTarget =
  | 'clash'
  | 'mihomo'
  | 'xray'
  | 'surge'
  | 'sing-box'
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
  | 'trojan'
  | 'mixed'
type ExportMode = 'strict' | 'best_effort'

interface CompatibilitySummary {
  supportedCount: number
  totalCount: number
  unsupportedNodes: NodeItem[]
}

const apiBase = import.meta.env.VITE_API_BASE_URL ?? 'http://127.0.0.1:8080'
const exportTargets: ExportTarget[] = [
  'xray',
  'clash',
  'mihomo',
  'surge',
  'sing-box',
  'surge3',
  'surge2',
  'quanx',
  'quan',
  'loon',
  'surfboard',
  'mellow',
  'clashr',
  'ss',
  'sssub',
  'ssr',
  'ssd',
  'trojan',
  'mixed',
]
const PAGE_SIZE_OPTIONS = [10, 20, 50, 100]

const TARGET_LABELS: Record<ExportTarget, string> = {
  clash: 'Clash',
  mihomo: 'Mihomo',
  xray: 'Xray',
  surge: 'Surge',
  'sing-box': 'sing-box',
  surge3: 'Surge 3',
  surge2: 'Surge 2',
  quanx: 'Quantumult X',
  quan: 'Quantumult',
  loon: 'Loon',
  surfboard: 'Surfboard',
  mellow: 'Mellow',
  clashr: 'ClashR',
  ss: 'SS SIP002',
  sssub: 'SS SIP008',
  ssr: 'SSR',
  ssd: 'SSD',
  trojan: 'Trojan URI',
  mixed: 'Mixed',
}

const MODE_LABELS: Record<ExportMode, string> = {
  strict: 'Strict',
  best_effort: 'Best Effort',
}

const TARGET_SUPPORT: Record<ExportTarget, Set<string>> = {
  clash: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2', 'tuic', 'wireguard', 'anytls']),
  mihomo: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2', 'tuic', 'wireguard', 'anytls']),
  xray: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2']),
  surge: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2', 'tuic', 'wireguard']),
  'sing-box': new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2', 'tuic', 'wireguard', 'anytls']),
  surge3: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2', 'tuic', 'wireguard']),
  surge2: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2', 'tuic', 'wireguard']),
  quanx: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2']),
  quan: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2', 'tuic', 'wireguard']),
  loon: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2', 'tuic', 'wireguard']),
  surfboard: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2', 'tuic', 'wireguard']),
  mellow: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2', 'tuic', 'wireguard', 'anytls']),
  clashr: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2', 'tuic', 'wireguard', 'anytls']),
  ss: new Set(['shadowsocks']),
  sssub: new Set(['shadowsocks']),
  ssr: new Set(['shadowsocks_r', 'ssr']),
  ssd: new Set(['shadowsocks']),
  trojan: new Set(['trojan']),
  mixed: new Set(['shadowsocks', 'vmess', 'vless', 'trojan', 'hysteria2', 'tuic', 'wireguard', 'anytls']),
}

const nodes = ref<NodeItem[]>([])
const subscriptions = ref<SubscriptionItem[]>([])
const templates = ref<TemplateItem[]>([])
const groups = ref<GroupItem[]>([])
const nodeGroups = ref<GroupItem[]>([])
const loading = ref(false)
const saving = ref(false)
const showEditor = ref(false)
const showGroupEditor = ref(false)
const editingId = ref<number | null>(null)
const editingGroupId = ref<number | null>(null)
const groupFilter = ref<number | 'all' | 'none'>('all')
const nodeGroupFilter = ref<number | 'all' | 'none'>('all')
const nodeHealthFilter = ref<'all' | 'ok'>('all')
const nodeSearch = ref('')
const selectedIds = ref<number[]>([])
const batchGroupId = ref<number | null>(null)
const page = ref(1)
const pageSize = ref(20)
const errorMessage = ref('')
const successMessage = ref('')
const exportMode = ref<ExportMode>('strict')
const compatibilityDetail = ref<{ title: string; message: string; nodes: NodeItem[] } | null>(null)
const exportConsole = ref<SubscriptionItem | null>(null)

const form = reactive({
  name: '',
  description: '',
  default_client: 'mihomo' as ExportTarget,
  template_id: null as number | null,
  group_id: null as number | null,
  enabled: true,
  expires_at: '',
  node_ids: [] as number[],
})

const groupForm = reactive({
  name: '',
  sort_order: 0,
})

const filteredSubscriptions = computed(() => {
  if (groupFilter.value === 'all') {
    return subscriptions.value
  }
  if (groupFilter.value === 'none') {
    return subscriptions.value.filter((item) => item.group_id === null)
  }
  return subscriptions.value.filter((item) => item.group_id === groupFilter.value)
})

const filteredFormNodes = computed(() => {
  let list = nodes.value
  if (nodeGroupFilter.value === 'none') {
    list = list.filter((item) => item.group_id === null)
  } else if (nodeGroupFilter.value !== 'all') {
    list = list.filter((item) => item.group_id === nodeGroupFilter.value)
  }

  if (nodeHealthFilter.value === 'ok') {
    list = list.filter((item) => item.last_latency_status === 'ok')
  }

  const keyword = nodeSearch.value.trim().toLowerCase()
  if (!keyword) {
    return list
  }

  return list.filter((item) =>
    [
      item.name,
      item.protocol,
      item.server,
      String(item.port),
      item.remark,
      nodeGroupName(item.group_id),
      item.last_latency_message ?? '',
    ]
      .join(' ')
      .toLowerCase()
      .includes(keyword),
  )
})

const pageCount = computed(() => Math.max(1, Math.ceil(filteredSubscriptions.value.length / pageSize.value)))
const pagedSubscriptions = computed(() => {
  const start = (page.value - 1) * pageSize.value
  return filteredSubscriptions.value.slice(start, start + pageSize.value)
})
const selectedSubscriptions = computed(() => subscriptions.value.filter((item) => selectedIds.value.includes(item.id)))
const currentPageIds = computed(() => pagedSubscriptions.value.map((item) => item.id))
const currentPageSelected = computed(
  () => currentPageIds.value.length > 0 && currentPageIds.value.every((id) => selectedIds.value.includes(id)),
)
const selectedCount = computed(() => selectedIds.value.length)
const selectedNodeCount = computed(() => form.node_ids.length)
const isEditing = computed(() => editingId.value !== null)
const isEditingGroup = computed(() => editingGroupId.value !== null)
const currentTargetLabel = computed(() => TARGET_LABELS[form.default_client])
const currentModeLabel = computed(() => MODE_LABELS[exportMode.value])
const selectedNodes = computed(() => nodes.value.filter((item) => form.node_ids.includes(item.id)))
const selectedFormNodes = computed(() => {
  const byId = new Map(nodes.value.map((item) => [item.id, item]))
  return form.node_ids.map((id) => byId.get(id)).filter((item): item is NodeItem => Boolean(item))
})
const missingSelectedNodeCount = computed(() => form.node_ids.length - selectedFormNodes.value.length)
const formCompatibility = computed(() => summarizeCompatibility(selectedNodes.value, form.default_client))
const selectedTemplate = computed(() => templates.value.find((item) => item.id === form.template_id) ?? null)
const usesUpstreamRawTemplate = computed(() => isUpstreamRawTemplate(selectedTemplate.value))
const canSubmitSubscription = computed(
  () => !saving.value && !!form.name && (form.node_ids.length > 0 || usesUpstreamRawTemplate.value),
)
const selectedTemplateMessage = computed(() => {
  if (!selectedTemplate.value) {
    return '当前订阅按默认导出器生成，不套用模板。'
  }

  if (usesUpstreamRawTemplate.value) {
    return '这是上游原始模板，可不选择节点，Clash/Mihomo 导出时会原样透传。'
  }

  return templateAppliesToTarget(selectedTemplate.value, form.default_client)
    ? `${selectedTemplate.value.kind} 模板会参与 ${currentTargetLabel.value} 导出。`
    : `${selectedTemplate.value.kind} 模板当前不会参与 ${currentTargetLabel.value} 导出。`
})
const submitLabel = computed(() => {
  if (saving.value) {
    return isEditing.value ? '保存中...' : '创建中...'
  }
  return isEditing.value ? '保存订阅' : '创建订阅'
})
const groupSubmitLabel = computed(() => (isEditingGroup.value ? '保存分组' : '创建分组'))

watch([filteredSubscriptions, pageSize], () => {
  page.value = Math.min(page.value, pageCount.value)
  selectedIds.value = selectedIds.value.filter((id) => filteredSubscriptions.value.some((item) => item.id === id))
})

watch(groupFilter, () => {
  page.value = 1
  selectedIds.value = []
})

function isNodeSupportedForTarget(node: Pick<NodeItem, 'protocol'>, target: ExportTarget) {
  return TARGET_SUPPORT[target].has(node.protocol)
}

function summarizeCompatibility(nodeItems: NodeItem[], target: ExportTarget): CompatibilitySummary {
  const unsupportedNodes = nodeItems.filter((item) => !isNodeSupportedForTarget(item, target))

  return {
    supportedCount: nodeItems.length - unsupportedNodes.length,
    totalCount: nodeItems.length,
    unsupportedNodes,
  }
}

function formatUnsupportedNodes(nodeItems: NodeItem[]) {
  return nodeItems.map((item) => `${item.name} (${item.protocol})`).join('、')
}

function compatibilityWarning(item: SubscriptionItem, target: ExportTarget) {
  const summary = subscriptionCompatibility(item, target)
  const count = summary.unsupportedNodes.length
  if (count === 0) {
    return ''
  }
  return exportMode.value === 'strict' ? `${count} 个节点不兼容，strict 会失败` : `${count} 个节点不兼容，best_effort 会跳过`
}

function openCompatibilityDetail(item: SubscriptionItem, target: ExportTarget) {
  const summary = subscriptionCompatibility(item, target)
  compatibilityDetail.value = {
    title: `${TARGET_LABELS[target]} 兼容详情`,
    message: compatibilityWarning(item, target),
    nodes: summary.unsupportedNodes,
  }
}

function subscriptionCompatibility(item: SubscriptionItem, target: ExportTarget) {
  return summarizeCompatibility(item.nodes, target)
}

function defaultTarget(item: SubscriptionItem): ExportTarget {
  return normalizeTarget(item.default_client)
}

function healthyExportCount(item: SubscriptionItem) {
  return exportTargets.filter((target) => subscriptionCompatibility(item, target).unsupportedNodes.length === 0).length
}

function warningExportCount(item: SubscriptionItem) {
  return exportTargets.length - healthyExportCount(item)
}

function openExportConsole(item: SubscriptionItem) {
  exportConsole.value = item
}

function exportLink(token: string, target: ExportTarget) {
  return `${apiBase}/s/${token}?target=${target}&mode=${exportMode.value}`
}

function autoSubscriptionLink(item: SubscriptionItem) {
  return `${apiBase}/s/${item.token}?mode=${exportMode.value}`
}

function subscriptionStatus(item: SubscriptionItem) {
  if (item.status) {
    return item.status
  }
  if (!item.enabled) {
    return 'disabled'
  }
  if (item.expires_at && new Date(item.expires_at).getTime() <= Date.now()) {
    return 'expired'
  }
  return 'active'
}

function subscriptionStatusLabel(item: SubscriptionItem) {
  const status = subscriptionStatus(item)
  if (status === 'expired') return '过期'
  if (status === 'disabled') return '停用'
  return '启用'
}

function subscriptionStatusClass(item: SubscriptionItem) {
  const status = subscriptionStatus(item)
  if (status === 'expired') return 'status-badge-warn'
  if (status === 'disabled') return 'status-badge-muted'
  return 'status-badge-neutral'
}

function formatExpiry(value: string | null) {
  if (!value) {
    return '长期有效'
  }
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) {
    return value
  }
  return date.toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
}

function toDateTimeLocal(value: string | null) {
  if (!value) {
    return ''
  }
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) {
    return ''
  }
  const offsetDate = new Date(date.getTime() - date.getTimezoneOffset() * 60_000)
  return offsetDate.toISOString().slice(0, 16)
}

function fromDateTimeLocal(value: string) {
  if (!value) {
    return null
  }
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? null : date.toISOString()
}

function setExpiryDays(days: number) {
  const date = new Date(Date.now() + days * 24 * 60 * 60 * 1000)
  const offsetDate = new Date(date.getTime() - date.getTimezoneOffset() * 60_000)
  form.expires_at = offsetDate.toISOString().slice(0, 16)
}

async function copyText(text: string, message: string) {
  try {
    await navigator.clipboard.writeText(text)
    successMessage.value = message
  } catch {
    errorMessage.value = '复制失败，请手动选中链接复制。'
  }
}

function normalizeTarget(target: string | null | undefined): ExportTarget {
  if (target && exportTargets.includes(target as ExportTarget)) {
    return target as ExportTarget
  }
  return 'mihomo'
}

function groupName(groupId: number | null) {
  if (groupId === null) {
    return '未分组'
  }
  return groups.value.find((item) => item.id === groupId)?.name ?? `分组 #${groupId}`
}

function nodeGroupName(groupId: number | null) {
  if (groupId === null) {
    return '未分组'
  }
  return nodeGroups.value.find((item) => item.id === groupId)?.name ?? `节点分组 #${groupId}`
}

function nodeLatencyText(item: NodeItem) {
  if (item.last_latency_status === 'ok' && item.last_latency_ms !== null) {
    return `${item.last_latency_ms} ms`
  }
  if (item.last_latency_status === 'timeout') {
    return '超时'
  }
  if (item.last_latency_status === 'error') {
    return '不可用'
  }
  return '未测速'
}

function nodeLatencyClass(item: NodeItem) {
  if (item.last_latency_status === 'ok') {
    return 'metric-chip-ok'
  }
  if (item.last_latency_status === 'timeout' || item.last_latency_status === 'error') {
    return 'metric-chip-warn'
  }
  return 'metric-chip-muted'
}

function nodeLatencyTitle(item: NodeItem) {
  const parts = [nodeLatencyText(item)]
  if (item.last_latency_tested_at) {
    parts.push(`最后测速：${formatExpiry(item.last_latency_tested_at)}`)
  }
  if (item.last_latency_message) {
    parts.push(item.last_latency_message)
  }
  return parts.join('\n')
}

function templateName(templateId: number | null) {
  if (templateId === null) {
    return '未指定模板'
  }
  return templates.value.find((item) => item.id === templateId)?.name ?? `模板 #${templateId}`
}

function isUpstreamRawTemplate(template: TemplateItem | null) {
  return template?.content.includes('x-sublinkx-upstream-template: true') ?? false
}

function templateAppliesToTarget(template: Pick<TemplateItem, 'kind'> | null, target: ExportTarget) {
  if (!template || target === 'xray' || target === 'ss' || target === 'sssub' || target === 'ssr' || target === 'ssd' || target === 'trojan' || target === 'mixed') {
    return false
  }
  if (target === 'clash' || target === 'clashr' || target === 'mellow') {
    return template.kind === 'common' || template.kind === 'clash' || template.kind === 'mihomo'
  }
  if (target === 'surge2' || target === 'surge3') {
    return template.kind === 'common' || template.kind === target || template.kind === 'surge'
  }
  return template.kind === 'common' || template.kind === target
}

function resetForm() {
  editingId.value = null
  form.name = ''
  form.description = ''
  form.default_client = 'mihomo'
  form.template_id = null
  form.group_id = null
  form.enabled = true
  form.expires_at = ''
  form.node_ids = []
  nodeGroupFilter.value = 'all'
  nodeHealthFilter.value = 'all'
  nodeSearch.value = ''
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

function startEdit(item: SubscriptionItem) {
  editingId.value = item.id
  form.name = item.name
  form.description = item.description
  form.default_client = normalizeTarget(item.default_client)
  form.template_id = item.template_id
  form.group_id = item.group_id
  form.enabled = item.enabled
  form.expires_at = toDateTimeLocal(item.expires_at)
  form.node_ids = [...item.node_ids]
  nodeGroupFilter.value = 'all'
  nodeHealthFilter.value = 'all'
  nodeSearch.value = ''
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

function toggleFilteredNodes(checked: boolean) {
  const filteredIds = filteredFormNodes.value.map((item) => item.id)
  if (checked) {
    form.node_ids = Array.from(new Set([...form.node_ids, ...filteredIds]))
    return
  }
  form.node_ids = form.node_ids.filter((id) => !filteredIds.includes(id))
}

function removeFormNode(nodeId: number) {
  form.node_ids = form.node_ids.filter((id) => id !== nodeId)
}

function clearFormNodes() {
  form.node_ids = []
}

function clearSelection() {
  selectedIds.value = []
}

async function load() {
  loading.value = true
  errorMessage.value = ''

  try {
    const [nodeResponse, subscriptionResponse, templateResponse, groupResponse, nodeGroupResponse] = await Promise.all([
      listNodes(),
      listSubscriptions(),
      listTemplates(),
      listSubscriptionGroups(),
      listNodeGroups(),
    ])

    nodes.value = nodeResponse.data
    subscriptions.value = subscriptionResponse.data
    templates.value = templateResponse.data
    groups.value = groupResponse.data
    nodeGroups.value = nodeGroupResponse.data
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
    const payload = {
      name: form.name,
      description: form.description || undefined,
      default_client: form.default_client,
      template_id: form.template_id,
      group_id: form.group_id,
      enabled: form.enabled,
      expires_at: fromDateTimeLocal(form.expires_at),
      node_ids: form.node_ids,
    }

    if (editingId.value !== null) {
      await updateSubscription(editingId.value, payload)
      successMessage.value = '订阅已更新。'
    } else {
      await createSubscription(payload)
      successMessage.value = '订阅已创建。'
    }

    closeEditor()
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    saving.value = false
  }
}

async function moveSelectedSubscriptions() {
  if (selectedSubscriptions.value.length === 0) {
    return
  }

  saving.value = true
  errorMessage.value = ''
  successMessage.value = ''

  try {
    await Promise.all(
      selectedSubscriptions.value.map((item) =>
        updateSubscription(item.id, {
          name: item.name,
          description: item.description || undefined,
          default_client: item.default_client,
          template_id: item.template_id,
          group_id: batchGroupId.value,
          enabled: item.enabled,
          expires_at: item.expires_at,
          node_ids: item.node_ids,
        }),
      ),
    )
    successMessage.value = `已移动 ${selectedSubscriptions.value.length} 个订阅到「${groupName(batchGroupId.value)}」。`
    clearSelection()
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
      await updateSubscriptionGroup(editingGroupId.value, payload)
      successMessage.value = '订阅分组已更新。'
    } else {
      await createSubscriptionGroup(payload)
      successMessage.value = '订阅分组已创建。'
    }
    resetGroupForm()
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    saving.value = false
  }
}

async function rotateToken(id: number) {
  try {
    await rotateSubscriptionToken(id)
    successMessage.value = '订阅 token 已轮换。'
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  }
}

async function toggleSubscriptionEnabled(item: SubscriptionItem) {
  try {
    await updateSubscription(item.id, {
      name: item.name,
      description: item.description || undefined,
      default_client: item.default_client,
      template_id: item.template_id,
      group_id: item.group_id,
      enabled: !item.enabled,
      expires_at: item.expires_at,
      node_ids: item.node_ids,
    })
    successMessage.value = item.enabled ? '订阅已停用。' : '订阅已启用。'
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  }
}

async function renew(item: SubscriptionItem, days = 30) {
  try {
    await renewSubscription(item.id, days)
    successMessage.value = `已为「${item.name}」续期 ${days} 天。`
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  }
}

async function removeSubscription(id: number) {
  if (!window.confirm('确定删除这个订阅吗？')) {
    return
  }

  try {
    await deleteSubscription(id)
    if (editingId.value === id) {
      closeEditor()
    }
    selectedIds.value = selectedIds.value.filter((item) => item !== id)
    successMessage.value = '订阅已删除。'
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  }
}

async function removeGroup(id: number) {
  if (!window.confirm('确定删除这个分组吗？')) {
    return
  }

  try {
    await deleteSubscriptionGroup(id)
    if (groupFilter.value === id) {
      groupFilter.value = 'all'
    }
    if (batchGroupId.value === id) {
      batchGroupId.value = null
    }
    successMessage.value = '订阅分组已删除。'
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
        <span class="eyebrow">Subscriptions</span>
        <h2 class="page-title">订阅管理</h2>
        <p class="page-copy">
          共 {{ subscriptions.length }} 个订阅，{{ groups.length }} 个分组，当前导出策略为 {{ currentModeLabel }}。
        </p>
      </div>
      <div class="inline-actions">
        <select v-model="groupFilter" class="select toolbar-select" aria-label="订阅分组筛选">
          <option value="all">全部分组</option>
          <option value="none">未分组</option>
          <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option>
        </select>
        <select v-model="exportMode" class="select toolbar-select" aria-label="导出策略">
          <option value="strict">strict</option>
          <option value="best_effort">best_effort</option>
        </select>
        <button class="button button-ghost" type="button" :disabled="loading" @click="load">
          {{ loading ? '刷新中...' : '刷新' }}
        </button>
        <button class="button button-ghost" type="button" @click="openGroupEditor">分组管理</button>
        <button class="button button-accent" type="button" @click="openCreate">创建订阅</button>
      </div>
    </header>

    <div v-if="errorMessage" class="error-banner">{{ errorMessage }}</div>
    <div v-if="successMessage" class="success-banner">{{ successMessage }}</div>

    <article class="card stack management-card">
      <div class="section-bar">
        <div>
          <div class="hint">订阅列表</div>
          <p class="card-copy">当前筛选 {{ filteredSubscriptions.length }} 个，已选择 {{ selectedCount }} 个。</p>
        </div>
        <div class="inline-actions bulk-actions">
          <select v-model="batchGroupId" class="select toolbar-select" aria-label="批量移动到分组">
            <option :value="null">移动到未分组</option>
            <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option>
          </select>
          <button class="button button-ghost" type="button" :disabled="selectedCount === 0" @click="clearSelection">清空选择</button>
          <button class="button button-accent" type="button" :disabled="saving || selectedCount === 0" @click="moveSelectedSubscriptions">
            移动分组
          </button>
        </div>
      </div>

      <div v-if="filteredSubscriptions.length === 0" class="empty-state">这个分组下还没有订阅。</div>

      <div v-else class="table-wrap subscription-board">
        <table class="table dense-table selectable-table subscription-table">
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
              <th>订阅</th>
              <th>出口</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="item in pagedSubscriptions"
              :key="item.id"
              :class="{ 'row-selected': selectedIds.includes(item.id) }"
            >
              <td class="selection-cell">
                <input v-model="selectedIds" :value="item.id" type="checkbox" :aria-label="`选择 ${item.name}`" />
              </td>
              <td class="subscription-name-cell">
                <div class="subscription-title-row">
                  <strong class="row-title">{{ item.name }}</strong>
                  <span class="status-badge" :class="subscriptionStatusClass(item)">
                    {{ subscriptionStatusLabel(item) }}
                  </span>
                </div>
                <div class="subscription-inline-meta">
                  <span>{{ item.description || '无描述' }}</span>
                  <code class="compact-token">{{ item.token }}</code>
                </div>
                <div class="subscription-chip-rail">
                  <span class="status-badge status-badge-neutral">{{ groupName(item.group_id) }}</span>
                  <span class="metric-chip">{{ item.node_ids.length }} 节点</span>
                  <span class="metric-chip" :class="{ 'metric-chip-warn': subscriptionStatus(item) === 'expired' }">
                    {{ formatExpiry(item.expires_at) }}
                  </span>
                  <span class="metric-chip template-chip" :title="templateName(item.template_id)">
                    {{ templateName(item.template_id) }}
                  </span>
                </div>
              </td>
              <td class="subscription-export-cell">
                <div class="inline-meter">
                  <span class="metric-chip">默认 {{ TARGET_LABELS[defaultTarget(item)] }}</span>
                  <span class="status-badge status-badge-ok">{{ healthyExportCount(item) }} 可用</span>
                  <span v-if="warningExportCount(item) > 0" class="status-badge status-badge-warn">
                    {{ warningExportCount(item) }} 警告
                  </span>
                  <button class="button button-ghost button-compact" type="button" @click="copyText(autoSubscriptionLink(item), '自动识别订阅链接已复制。')">
                    复制
                  </button>
                  <button class="button button-accent button-compact" type="button" @click="openExportConsole(item)">
                    导出
                  </button>
                </div>
              </td>
              <td>
                <div class="inline-actions row-actions">
                  <button class="button button-ghost button-compact" type="button" @click="startEdit(item)">编辑</button>
                  <button class="button button-ghost button-compact" type="button" @click="toggleSubscriptionEnabled(item)">
                    {{ item.enabled ? '停用' : '启用' }}
                  </button>
                  <button class="button button-ghost button-compact" type="button" @click="renew(item, 30)">续30天</button>
                  <button class="button button-ghost button-compact" type="button" @click="rotateToken(item.id)">轮换</button>
                  <button class="button button-danger button-compact" type="button" @click="removeSubscription(item.id)">删除</button>
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
        <section class="modal-panel modal-panel-wide">
          <header class="modal-header">
            <div>
              <span class="eyebrow">{{ isEditing ? 'Edit Subscription' : 'New Subscription' }}</span>
              <h3>{{ isEditing ? '编辑订阅' : '创建订阅' }}</h3>
            </div>
            <button class="icon-button" type="button" aria-label="关闭" @click="closeEditor">×</button>
          </header>

          <form class="form-grid" @submit.prevent="submit">
            <div class="form-columns">
              <div class="stack">
                <div>
                  <label class="field-label" for="subscription-name">订阅名称</label>
                  <input id="subscription-name" v-model.trim="form.name" class="input" placeholder="例如：主力线路" />
                </div>

                <div>
                  <label class="field-label" for="subscription-group">订阅分组</label>
                  <select id="subscription-group" v-model="form.group_id" class="select">
                    <option :value="null">未分组</option>
                    <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option>
                  </select>
                </div>

                <div>
                  <label class="field-label" for="subscription-description">描述</label>
                  <textarea id="subscription-description" v-model.trim="form.description" class="textarea" placeholder="可选" />
                </div>

                <div>
                  <label class="field-label" for="subscription-client">默认客户端</label>
                  <select id="subscription-client" v-model="form.default_client" class="select">
                    <option v-for="target in exportTargets" :key="target" :value="target">
                      {{ TARGET_LABELS[target] }}
                    </option>
                  </select>
                </div>

                <div>
                  <label class="field-label" for="subscription-expires-at">到期时间</label>
                  <input id="subscription-expires-at" v-model="form.expires_at" class="input" type="datetime-local" />
                  <div class="quick-expiry-row">
                    <button class="button button-ghost button-compact" type="button" @click="setExpiryDays(15)">15天</button>
                    <button class="button button-ghost button-compact" type="button" @click="setExpiryDays(30)">30天</button>
                    <button class="button button-ghost button-compact" type="button" @click="setExpiryDays(90)">90天</button>
                    <button class="button button-ghost button-compact" type="button" @click="setExpiryDays(180)">180天</button>
                    <button class="button button-ghost button-compact" type="button" @click="setExpiryDays(365)">365天</button>
                    <button class="button button-ghost button-compact" type="button" @click="form.expires_at = ''">长期</button>
                  </div>
                  <div class="hint template-kind-hint">留空表示长期有效；过期后订阅链接会停止导出。</div>
                </div>

                <div>
                  <label class="field-label" for="subscription-template">关联模板</label>
                  <select id="subscription-template" v-model="form.template_id" class="select">
                    <option :value="null">不使用模板</option>
                    <option v-for="item in templates" :key="item.id" :value="item.id">
                      {{ item.name }} · {{ item.kind }}
                    </option>
                  </select>
                  <div class="hint template-kind-hint">{{ selectedTemplateMessage }}</div>
                </div>
              </div>

              <div class="stack">
                <label class="checkbox-item">
                  <input v-model="form.enabled" type="checkbox" />
                  <span>
                    <strong>启用订阅</strong>
                    <div class="muted">关闭后保留记录，但不建议继续分发给客户端。</div>
                  </span>
                </label>

                <div v-if="usesUpstreamRawTemplate" class="compat-panel">
                  <div class="inline-actions">
                    <span class="status-badge status-badge-ok">RAW TEMPLATE</span>
                    <span class="status-badge status-badge-neutral">Clash/Mihomo</span>
                  </div>
                  <p class="compat-copy compat-copy-ok">
                    当前订阅会直接使用上游原始模板导出，可以不选择节点。
                  </p>
                </div>

                <div v-else class="compat-panel">
                  <div class="inline-actions">
                    <span class="status-badge" :class="formCompatibility.unsupportedNodes.length === 0 ? 'status-badge-ok' : 'status-badge-warn'">
                      {{ currentTargetLabel }}
                    </span>
                    <span class="status-badge status-badge-neutral">{{ currentModeLabel }}</span>
                    <span class="hint">{{ formCompatibility.supportedCount }}/{{ formCompatibility.totalCount }} 可导出</span>
                  </div>
                  <p v-if="formCompatibility.unsupportedNodes.length > 0" class="compat-copy compat-copy-warn">
                    {{ formatUnsupportedNodes(formCompatibility.unsupportedNodes) }}
                  </p>
                </div>

                <div v-if="!usesUpstreamRawTemplate" class="stack compact-stack">
                  <div class="selected-node-panel">
                    <div class="selected-node-panel-head">
                      <div>
                        <strong>已选节点</strong>
                        <span class="hint">
                          {{ selectedNodeCount }} 个
                          <template v-if="missingSelectedNodeCount > 0">，{{ missingSelectedNodeCount }} 个节点已不存在</template>
                        </span>
                      </div>
                      <button
                        class="button button-ghost button-compact"
                        type="button"
                        :disabled="selectedNodeCount === 0"
                        @click="clearFormNodes"
                      >
                        清空已选
                      </button>
                    </div>
                    <div v-if="selectedFormNodes.length === 0" class="hint">还没有选择节点，可以在下方筛选后勾选。</div>
                    <div v-else class="selected-node-chip-list">
                      <span v-for="item in selectedFormNodes" :key="item.id" class="selected-node-chip">
                        <span class="selected-node-chip-main">
                          <strong>{{ item.name }}</strong>
                          <small>{{ item.protocol }} · {{ nodeLatencyText(item) }}</small>
                        </span>
                        <button type="button" aria-label="移除节点" @click="removeFormNode(item.id)">×</button>
                      </span>
                    </div>
                  </div>

                  <div class="node-picker-toolbar">
                    <div>
                      <label class="field-label" for="subscription-node-group">节点分组筛选</label>
                      <select id="subscription-node-group" v-model="nodeGroupFilter" class="select">
                        <option value="all">全部节点分组</option>
                        <option value="none">未分组节点</option>
                        <option v-for="group in nodeGroups" :key="group.id" :value="group.id">{{ group.name }}</option>
                      </select>
                    </div>
                    <div>
                      <label class="field-label" for="subscription-node-health">链接状态</label>
                      <select id="subscription-node-health" v-model="nodeHealthFilter" class="select">
                        <option value="all">全部显示</option>
                        <option value="ok">只显示正常链接</option>
                      </select>
                    </div>
                    <div>
                      <label class="field-label" for="subscription-node-search">搜索节点</label>
                      <input
                        id="subscription-node-search"
                        v-model.trim="nodeSearch"
                        class="input"
                        type="search"
                        placeholder="名称 / 协议 / 地址 / 分组"
                      />
                    </div>
                    <button
                      class="button button-ghost button-compact"
                      type="button"
                      :disabled="filteredFormNodes.length === 0"
                      @click="toggleFilteredNodes(true)"
                    >
                      选中当前分组
                    </button>
                    <button
                      class="button button-ghost button-compact"
                      type="button"
                      :disabled="filteredFormNodes.length === 0"
                      @click="toggleFilteredNodes(false)"
                    >
                      取消当前分组
                    </button>
                  </div>

                  <div class="field-label">
                    选择节点，已选 {{ selectedNodeCount }} 个，当前显示 {{ filteredFormNodes.length }} 个
                  </div>
                  <div v-if="nodes.length === 0" class="empty-state">还没有节点，请先导入节点。</div>
                  <div v-else-if="filteredFormNodes.length === 0" class="empty-state">没有匹配的节点，可以放宽分组、状态或搜索条件。</div>
                  <div v-else class="checkbox-list modal-node-list">
                    <label v-for="item in filteredFormNodes" :key="item.id" class="checkbox-item">
                      <input v-model="form.node_ids" :value="item.id" type="checkbox" />
                      <span class="node-option-body">
                        <span class="node-option-title">
                          <strong>{{ item.name }}</strong>
                          <span class="metric-chip node-latency-chip" :class="nodeLatencyClass(item)" :title="nodeLatencyTitle(item)">
                            {{ nodeLatencyText(item) }}
                          </span>
                        </span>
                        <div class="muted">
                          {{ nodeGroupName(item.group_id) }} · {{ item.protocol }} · {{ item.server }}:{{ item.port }}
                        </div>
                      </span>
                    </label>
                  </div>
                </div>
              </div>
            </div>

            <div class="modal-actions">
              <button class="button button-ghost" type="button" :disabled="saving" @click="closeEditor">取消</button>
              <button class="button button-accent" type="submit" :disabled="!canSubmitSubscription">
                {{ submitLabel }}
              </button>
            </div>
          </form>
        </section>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-if="exportConsole" class="modal-backdrop" @click.self="exportConsole = null">
        <section class="modal-panel modal-panel-wide export-console-panel">
          <header class="modal-header">
            <div>
              <span class="eyebrow">Export Console</span>
              <h3>{{ exportConsole.name }}</h3>
              <p class="card-copy">
                {{ exportTargets.length }} 个客户端出口，当前策略为 {{ currentModeLabel }}。自动识别链接会按客户端 User-Agent 选择导出格式。
              </p>
            </div>
            <button class="icon-button" type="button" aria-label="关闭" @click="exportConsole = null">x</button>
          </header>

          <div class="export-console-hero">
            <div>
              <div class="hint">自动识别订阅</div>
              <a class="token-link export-console-auto-link" :href="autoSubscriptionLink(exportConsole)" target="_blank" rel="noreferrer">
                {{ autoSubscriptionLink(exportConsole) }}
              </a>
            </div>
            <button
              class="button button-accent"
              type="button"
              @click="copyText(autoSubscriptionLink(exportConsole), '自动识别订阅链接已复制。')"
            >
              复制自动链接
            </button>
          </div>

          <div class="export-console-grid">
            <article v-for="target in exportTargets" :key="target" class="export-console-card">
              <div class="export-console-card-head">
                <div>
                  <div class="hint">客户端</div>
                  <strong>{{ TARGET_LABELS[target] }}</strong>
                </div>
                <span
                  class="status-badge"
                  :class="subscriptionCompatibility(exportConsole, target).unsupportedNodes.length === 0 ? 'status-badge-ok' : 'status-badge-warn'"
                >
                  {{ subscriptionCompatibility(exportConsole, target).supportedCount }}/{{ subscriptionCompatibility(exportConsole, target).totalCount }}
                </span>
              </div>
              <a class="token-link export-console-link" :href="exportLink(exportConsole.token, target)" target="_blank" rel="noreferrer">
                {{ exportLink(exportConsole.token, target) }}
              </a>
              <div class="export-console-actions">
                <button
                  class="button button-ghost button-compact"
                  type="button"
                  @click="copyText(exportLink(exportConsole.token, target), `${TARGET_LABELS[target]} 订阅链接已复制。`)"
                >
                  复制链接
                </button>
                <button
                  v-if="subscriptionCompatibility(exportConsole, target).unsupportedNodes.length > 0"
                  class="button button-ghost button-compact"
                  type="button"
                  @click="openCompatibilityDetail(exportConsole, target)"
                >
                  查看警告
                </button>
              </div>
            </article>
          </div>
        </section>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-if="compatibilityDetail" class="modal-backdrop" @click.self="compatibilityDetail = null">
        <section class="modal-panel">
          <header class="modal-header">
            <div>
              <span class="eyebrow">Compatibility</span>
              <h3>{{ compatibilityDetail.title }}</h3>
            </div>
            <button class="icon-button" type="button" aria-label="关闭" @click="compatibilityDetail = null">×</button>
          </header>

          <div class="stack">
            <p class="card-copy">{{ compatibilityDetail.message }}</p>
            <div class="checkbox-list modal-node-list">
              <div v-for="node in compatibilityDetail.nodes" :key="node.id" class="checkbox-item">
                <span>
                  <strong>{{ node.name }}</strong>
                  <div class="muted">{{ node.protocol }} · {{ node.server }}:{{ node.port }}</div>
                </span>
              </div>
            </div>
          </div>
        </section>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-if="showGroupEditor" class="modal-backdrop" @click.self="closeGroupEditor">
        <section class="modal-panel">
          <header class="modal-header">
            <div>
              <span class="eyebrow">Subscription Groups</span>
              <h3>订阅分组</h3>
            </div>
            <button class="icon-button" type="button" aria-label="关闭" @click="closeGroupEditor">×</button>
          </header>

          <form class="form-grid" @submit.prevent="submitGroup">
            <div>
              <label class="field-label" for="subscription-group-name">分组名称</label>
              <input id="subscription-group-name" v-model.trim="groupForm.name" class="input" placeholder="例如：影视订阅" />
            </div>
            <div>
              <label class="field-label" for="subscription-group-order">排序</label>
              <input id="subscription-group-order" v-model.number="groupForm.sort_order" class="input" type="number" />
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
