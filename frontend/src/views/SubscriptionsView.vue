<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { extractApiError } from '../api/client'
import { useI18n } from '../i18n'
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
import { readStoredPageSize, storePageSize } from '../utils/pagination'

const { t } = useI18n()

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
const PAGE_SIZE_STORAGE_KEY = 'sublinkx_subscriptions_page_size'

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
const displayMode = ref<'detailed' | 'minimal'>('detailed')
const detailSubscription = ref<SubscriptionItem | null>(null)
const page = ref(1)
const pageSize = ref(readStoredPageSize(PAGE_SIZE_STORAGE_KEY, PAGE_SIZE_OPTIONS))
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
    return t('defaultExporterTemplateMessage')
  }

  if (usesUpstreamRawTemplate.value) {
    return t('upstreamRawTemplateMessage')
  }

  return templateAppliesToTarget(selectedTemplate.value, form.default_client)
    ? t('templateApplies', { kind: selectedTemplate.value.kind, target: currentTargetLabel.value })
    : t('templateNotApplies', { kind: selectedTemplate.value.kind, target: currentTargetLabel.value })
})
const submitLabel = computed(() => {
  if (saving.value) {
    return isEditing.value ? t('saving') : t('creating')
  }
  return isEditing.value ? t('saveSubscription') : t('createSubscription')
})
const groupSubmitLabel = computed(() => (isEditingGroup.value ? t('saveGroup') : t('createGroup')))

watch([filteredSubscriptions, pageSize], () => {
  page.value = Math.min(page.value, pageCount.value)
  selectedIds.value = selectedIds.value.filter((id) => filteredSubscriptions.value.some((item) => item.id === id))
})

watch(pageSize, (value) => {
  storePageSize(PAGE_SIZE_STORAGE_KEY, value)
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
  return exportMode.value === 'strict' ? t('incompatibleStrict', { count }) : t('incompatibleBestEffort', { count })
}

function openCompatibilityDetail(item: SubscriptionItem, target: ExportTarget) {
  const summary = subscriptionCompatibility(item, target)
  compatibilityDetail.value = {
    title: t('compatibilityDetail', { target: TARGET_LABELS[target] }),
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
  if (status === 'expired') return t('expired')
  if (status === 'disabled') return t('disabled')
  return t('active')
}

function subscriptionStatusClass(item: SubscriptionItem) {
  const status = subscriptionStatus(item)
  if (status === 'expired') return 'status-badge-warn'
  if (status === 'disabled') return 'status-badge-muted'
  return 'status-badge-neutral'
}

function formatExpiry(value: string | null) {
  if (!value) {
    return t('longTerm')
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
    errorMessage.value = t('copyFailed')
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
    return t('ungrouped')
  }
  return groups.value.find((item) => item.id === groupId)?.name ?? t('groupFallback', { id: groupId })
}

function nodeGroupName(groupId: number | null) {
  if (groupId === null) {
    return t('ungrouped')
  }
  return nodeGroups.value.find((item) => item.id === groupId)?.name ?? t('nodeGroupFallback', { id: groupId })
}

function nodeLatencyText(item: NodeItem) {
  if (item.last_latency_status === 'ok' && item.last_latency_ms !== null) {
    return `${item.last_latency_ms} ms`
  }
  if (item.last_latency_status === 'timeout') {
    return t('timeout')
  }
  if (item.last_latency_status === 'error') {
    return t('unavailable')
  }
  return t('untested')
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
    parts.push(t('lastLatency', { time: formatExpiry(item.last_latency_tested_at) }))
  }
  if (item.last_latency_message) {
    parts.push(item.last_latency_message)
  }
  return parts.join('\n')
}

function templateName(templateId: number | null) {
  if (templateId === null) {
    return t('unspecifiedTemplate')
  }
  return templates.value.find((item) => item.id === templateId)?.name ?? t('templateFallback', { id: templateId })
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

function upsertSubscription(item: SubscriptionItem) {
  const index = subscriptions.value.findIndex((subscription) => subscription.id === item.id)
  if (index >= 0) {
    subscriptions.value[index] = item
    return
  }
  subscriptions.value = [item, ...subscriptions.value]
}

function removeSubscriptionById(id: number) {
  subscriptions.value = subscriptions.value.filter((item) => item.id !== id)
  selectedIds.value = selectedIds.value.filter((item) => item !== id)
}

function upsertGroup(item: GroupItem) {
  const index = groups.value.findIndex((group) => group.id === item.id)
  if (index >= 0) {
    groups.value[index] = item
    return
  }
  groups.value = [...groups.value, item].sort((left, right) => left.sort_order - right.sort_order || left.id - right.id)
}

function removeGroupById(id: number) {
  groups.value = groups.value.filter((item) => item.id !== id)
  subscriptions.value = subscriptions.value.map((item) => (item.group_id === id ? { ...item, group_id: null } : item))
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

  const [nodeResponse, subscriptionResponse, templateResponse, groupResponse, nodeGroupResponse] = await Promise.allSettled([
    listNodes(),
    listSubscriptions(),
    listTemplates(),
    listSubscriptionGroups(),
    listNodeGroups(),
  ])

  if (nodeResponse.status === 'fulfilled') {
    nodes.value = nodeResponse.value.data
  }
  if (subscriptionResponse.status === 'fulfilled') {
    subscriptions.value = subscriptionResponse.value.data
  }
  if (templateResponse.status === 'fulfilled') {
    templates.value = templateResponse.value.data
  }
  if (groupResponse.status === 'fulfilled') {
    groups.value = groupResponse.value.data
  }
  if (nodeGroupResponse.status === 'fulfilled') {
    nodeGroups.value = nodeGroupResponse.value.data
    page.value = Math.min(page.value, pageCount.value)
  }
  const failure = [nodeResponse, subscriptionResponse, templateResponse, groupResponse, nodeGroupResponse].find(
    (result) => result.status === 'rejected',
  )
  if (failure?.status === 'rejected') {
    errorMessage.value = extractApiError(failure.reason)
  }
  loading.value = false
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
      const response = await updateSubscription(editingId.value, payload)
      upsertSubscription(response.data)
      successMessage.value = t('subscriptionUpdated')
    } else {
      const response = await createSubscription(payload)
      upsertSubscription(response.data)
      successMessage.value = t('subscriptionCreated')
    }

    closeEditor()
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
    successMessage.value = t('subscriptionsMoved', {
      count: selectedSubscriptions.value.length,
      group: groupName(batchGroupId.value),
    })
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
      const response = await updateSubscriptionGroup(editingGroupId.value, payload)
      upsertGroup(response.data)
      successMessage.value = t('subscriptionGroupUpdated')
    } else {
      const response = await createSubscriptionGroup(payload)
      upsertGroup(response.data)
      successMessage.value = t('subscriptionGroupCreated')
    }
    resetGroupForm()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    saving.value = false
  }
}

async function rotateToken(id: number) {
  try {
    await rotateSubscriptionToken(id)
    successMessage.value = t('tokenRotated')
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
    successMessage.value = item.enabled ? t('subscriptionDisabled') : t('subscriptionEnabled')
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  }
}

async function renew(item: SubscriptionItem, days = 30) {
  try {
    await renewSubscription(item.id, days)
    successMessage.value = t('renewedDays', { name: item.name, days })
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  }
}

async function removeSubscription(id: number) {
  if (!window.confirm(t('confirmDeleteSubscription'))) {
    return
  }

  try {
    await deleteSubscription(id)
    if (editingId.value === id) {
      closeEditor()
    }
    removeSubscriptionById(id)
    successMessage.value = t('subscriptionDeleted')
  } catch (error) {
    errorMessage.value = extractApiError(error)
  }
}

async function removeGroup(id: number) {
  if (!window.confirm(t('confirmDeleteSubscriptionGroup'))) {
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
    removeGroupById(id)
    successMessage.value = t('subscriptionGroupDeleted')
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
        <h2 class="page-title">{{ t('subscriptions') }}</h2>
        <p class="page-copy">{{ t('subscriptionsCopy') }}</p>
      </div>
      <div class="inline-actions page-actions">
        <select v-model="groupFilter" class="select toolbar-select" :aria-label="t('subscriptionGroup')">
          <option value="all">{{ t('allGroups') }}</option>
          <option value="none">{{ t('ungrouped') }}</option>
          <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option>
        </select>
        <select v-model="exportMode" class="select toolbar-select" :aria-label="t('exportStrategy')">
          <option value="strict">strict</option>
          <option value="best_effort">best_effort</option>
        </select>
        <button class="button button-ghost mobile-secondary-action" type="button" :disabled="loading" @click="load">
          {{ loading ? t('refreshing') : t('refresh') }}
        </button>
        <button class="button button-ghost mobile-secondary-action" type="button" @click="openGroupEditor">{{ t('groupManagement') }}</button>
        <details class="mobile-action-drawer">
          <summary>{{ t('moreActions') }}</summary>
          <div class="mobile-action-grid">
            <button class="button button-ghost" type="button" :disabled="loading" @click="load">
              {{ loading ? t('refreshing') : t('refresh') }}
            </button>
            <button class="button button-ghost" type="button" @click="openGroupEditor">{{ t('groupManagement') }}</button>
          </div>
        </details>
        <button class="button button-accent" type="button" @click="openCreate">{{ t('createSubscription') }}</button>
      </div>
    </header>

    <div v-if="errorMessage" class="error-banner">{{ errorMessage }}</div>
    <div v-if="successMessage" class="success-banner">{{ successMessage }}</div>

    <article class="card stack management-card">
      <div class="section-bar">
        <div>
          <div class="hint">{{ t('subscriptionList') }}</div>
          <p class="card-copy">{{ t('subscriptionListSummary', { filtered: filteredSubscriptions.length, selected: selectedCount }) }}</p>
        </div>
        <div class="view-switch">
          <button class="view-switch-button" :class="{ active: displayMode === 'detailed' }" type="button" @click="displayMode = 'detailed'">
            {{ t('detailedView') }}
          </button>
          <button class="view-switch-button" :class="{ active: displayMode === 'minimal' }" type="button" @click="displayMode = 'minimal'">
            {{ t('minimalView') }}
          </button>
        </div>
        <label v-if="filteredSubscriptions.length > 0" class="select-page-toggle" :class="{ 'is-active': currentPageSelected }">
          <input
            type="checkbox"
            :checked="currentPageSelected"
            @change="toggleCurrentPage(($event.target as HTMLInputElement).checked)"
          />
          <span class="select-page-toggle-mark"></span>
          <span class="select-page-toggle-text">{{ currentPageSelected ? t('unselectCurrentPage') : t('selectCurrentPage') }}</span>
          <span class="select-page-toggle-count">{{ selectedCount }}/{{ pagedSubscriptions.length }}</span>
        </label>
        <div class="inline-actions bulk-actions" :class="{ 'is-empty-selection': selectedCount === 0 }">
          <select v-model="batchGroupId" class="select toolbar-select" :aria-label="t('moveGroup')">
            <option :value="null">{{ t('moveToUngrouped') }}</option>
            <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option>
          </select>
          <button class="button button-ghost" type="button" :disabled="selectedCount === 0" @click="clearSelection">{{ t('clearSelection') }}</button>
          <button class="button button-accent" type="button" :disabled="saving || selectedCount === 0" @click="moveSelectedSubscriptions">
            {{ t('moveGroup') }}
          </button>
        </div>
      </div>

      <div v-if="filteredSubscriptions.length === 0" class="empty-state">{{ t('emptySubscriptions') }}</div>

      <div v-else-if="displayMode === 'minimal'" class="minimal-card-grid">
        <div v-for="item in pagedSubscriptions" :key="item.id" class="minimal-info-card" :class="{ 'is-selected': selectedIds.includes(item.id) }">
          <label class="minimal-select-dot" :aria-label="t('selectSubscription', { name: item.name })" @click.stop>
            <input v-model="selectedIds" :value="item.id" type="checkbox" />
            <span></span>
          </label>
          <button class="minimal-card-main" type="button" @click="detailSubscription = item">
            <span class="minimal-card-title">{{ item.name }}</span>
            <span class="metric-chip" :class="{ 'metric-chip-warn': subscriptionStatus(item) === 'expired' }">
              {{ formatExpiry(item.expires_at) }}
            </span>
          </button>
        </div>
      </div>

      <div v-else class="table-wrap subscription-board">
        <table class="table dense-table selectable-table subscription-table">
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
              <th>{{ t('subscription') }}</th>
              <th>{{ t('outlet') }}</th>
              <th>{{ t('actions') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="item in pagedSubscriptions"
              :key="item.id"
              :class="{ 'row-selected': selectedIds.includes(item.id) }"
            >
              <td class="selection-cell">
                <input v-model="selectedIds" :value="item.id" type="checkbox" :aria-label="t('selectSubscription', { name: item.name })" />
              </td>
              <td class="subscription-name-cell">
                <div class="subscription-title-row">
                  <strong class="row-title">{{ item.name }}</strong>
                  <span class="status-badge" :class="subscriptionStatusClass(item)">
                    {{ subscriptionStatusLabel(item) }}
                  </span>
                </div>
                <div class="subscription-inline-meta">
                  <span>{{ item.description || t('noDescription') }}</span>
                  <code class="compact-token">{{ item.token }}</code>
                </div>
                <div class="subscription-chip-rail">
                  <span class="status-badge status-badge-neutral">{{ groupName(item.group_id) }}</span>
                  <span class="metric-chip">{{ t('nodesUnit', { count: item.node_ids.length }) }}</span>
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
                  <span class="metric-chip">{{ t('defaultClient', { client: TARGET_LABELS[defaultTarget(item)] }) }}</span>
                  <span class="status-badge status-badge-ok">{{ t('available', { count: healthyExportCount(item) }) }}</span>
                  <span v-if="warningExportCount(item) > 0" class="status-badge status-badge-warn">
                    {{ t('warnings', { count: warningExportCount(item) }) }}
                  </span>
                  <button class="button button-ghost button-compact" type="button" @click="copyText(autoSubscriptionLink(item), t('autoLinkCopied'))">
                    {{ t('copy') }}
                  </button>
                  <button class="button button-accent button-compact" type="button" @click="openExportConsole(item)">
                    {{ t('export') }}
                  </button>
                </div>
              </td>
              <td>
                <div class="inline-actions row-actions">
                  <button class="button button-ghost button-compact" type="button" @click="startEdit(item)">{{ t('edit') }}</button>
                  <button class="button button-ghost button-compact" type="button" @click="toggleSubscriptionEnabled(item)">
                    {{ item.enabled ? t('disabled') : t('enabled') }}
                  </button>
                  <button class="button button-ghost button-compact" type="button" @click="renew(item, 30)">{{ t('renew30Days') }}</button>
                  <button class="button button-ghost button-compact" type="button" @click="rotateToken(item.id)">{{ t('rotate') }}</button>
                  <button class="button button-danger button-compact" type="button" @click="removeSubscription(item.id)">{{ t('delete') }}</button>
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
      <div v-if="detailSubscription" class="modal-backdrop" @click.self="detailSubscription = null">
        <section class="modal-panel">
          <header class="modal-header">
            <div>
              <span class="eyebrow">{{ t('subscriptionDetail') }}</span>
              <h3>{{ detailSubscription.name }}</h3>
            </div>
            <button class="icon-button" type="button" :aria-label="t('close')" @click="detailSubscription = null">×</button>
          </header>
          <div class="detail-grid">
            <div><span>{{ t('subscriptionGroup') }}</span><strong>{{ groupName(detailSubscription.group_id) }}</strong></div>
            <div><span>{{ t('expiresAt') }}</span><strong>{{ formatExpiry(detailSubscription.expires_at) }}</strong></div>
            <div><span>{{ t('node') }}</span><strong>{{ t('nodesUnit', { count: detailSubscription.node_ids.length }) }}</strong></div>
            <div><span>{{ t('defaultClientLabel') }}</span><strong>{{ TARGET_LABELS[defaultTarget(detailSubscription)] }}</strong></div>
          </div>
          <div class="modal-actions">
            <button class="button button-ghost" type="button" @click="copyText(autoSubscriptionLink(detailSubscription), t('autoLinkCopied'))">{{ t('copyAutoLink') }}</button>
            <button class="button button-accent" type="button" @click="openExportConsole(detailSubscription); detailSubscription = null">{{ t('export') }}</button>
            <button class="button button-ghost" type="button" @click="startEdit(detailSubscription); detailSubscription = null">{{ t('edit') }}</button>
          </div>
        </section>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-if="showEditor" class="modal-backdrop" @click.self="closeEditor">
        <section class="modal-panel modal-panel-wide">
          <header class="modal-header">
            <div>
              <span class="eyebrow">{{ isEditing ? 'Edit Subscription' : 'New Subscription' }}</span>
              <h3>{{ isEditing ? t('editSubscription') : t('createSubscription') }}</h3>
            </div>
            <button class="icon-button" type="button" :aria-label="t('close')" @click="closeEditor">×</button>
          </header>

          <form class="form-grid" @submit.prevent="submit">
            <div class="form-columns">
              <div class="stack">
                <div>
                  <label class="field-label" for="subscription-name">{{ t('subscriptionName') }}</label>
                  <input id="subscription-name" v-model.trim="form.name" class="input" :placeholder="t('subscriptionNamePlaceholder')" />
                </div>

                <div>
                  <label class="field-label" for="subscription-group">{{ t('subscriptionGroup') }}</label>
                  <select id="subscription-group" v-model="form.group_id" class="select">
                    <option :value="null">{{ t('ungrouped') }}</option>
                    <option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</option>
                  </select>
                </div>

                <div>
                  <label class="field-label" for="subscription-description">{{ t('description') }}</label>
                  <textarea id="subscription-description" v-model.trim="form.description" class="textarea" :placeholder="t('optional')" />
                </div>

                <div>
                  <label class="field-label" for="subscription-client">{{ t('defaultClientLabel') }}</label>
                  <select id="subscription-client" v-model="form.default_client" class="select">
                    <option v-for="target in exportTargets" :key="target" :value="target">
                      {{ TARGET_LABELS[target] }}
                    </option>
                  </select>
                </div>

                <div>
                  <label class="field-label" for="subscription-expires-at">{{ t('expiresAt') }}</label>
                  <input id="subscription-expires-at" v-model="form.expires_at" class="input" type="datetime-local" />
                  <div class="quick-expiry-row">
                    <button class="button button-ghost button-compact" type="button" @click="setExpiryDays(15)">{{ t('days15') }}</button>
                    <button class="button button-ghost button-compact" type="button" @click="setExpiryDays(30)">{{ t('days30') }}</button>
                    <button class="button button-ghost button-compact" type="button" @click="setExpiryDays(90)">{{ t('days90') }}</button>
                    <button class="button button-ghost button-compact" type="button" @click="setExpiryDays(180)">{{ t('days180') }}</button>
                    <button class="button button-ghost button-compact" type="button" @click="setExpiryDays(365)">{{ t('days365') }}</button>
                    <button class="button button-ghost button-compact" type="button" @click="form.expires_at = ''">{{ t('longTermButton') }}</button>
                  </div>
                  <div class="hint template-kind-hint">{{ t('expiryHint') }}</div>
                </div>

                <div>
                  <label class="field-label" for="subscription-template">{{ t('templateLabel') }}</label>
                  <select id="subscription-template" v-model="form.template_id" class="select">
                    <option :value="null">{{ t('noTemplate') }}</option>
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
                    <strong>{{ t('enableSubscription') }}</strong>
                    <div class="muted">{{ t('enableSubscriptionHint') }}</div>
                  </span>
                </label>

                <div v-if="usesUpstreamRawTemplate" class="compat-panel">
                  <div class="inline-actions">
                    <span class="status-badge status-badge-ok">RAW TEMPLATE</span>
                    <span class="status-badge status-badge-neutral">Clash/Mihomo</span>
                  </div>
                  <p class="compat-copy compat-copy-ok">
                    {{ t('rawTemplateExportHint') }}
                  </p>
                </div>

                <div v-else class="compat-panel">
                  <div class="inline-actions">
                    <span class="status-badge" :class="formCompatibility.unsupportedNodes.length === 0 ? 'status-badge-ok' : 'status-badge-warn'">
                      {{ currentTargetLabel }}
                    </span>
                    <span class="status-badge status-badge-neutral">{{ currentModeLabel }}</span>
                    <span class="hint">{{ formCompatibility.supportedCount }}/{{ formCompatibility.totalCount }} {{ t('exportable') }}</span>
                  </div>
                  <p v-if="formCompatibility.unsupportedNodes.length > 0" class="compat-copy compat-copy-warn">
                    {{ formatUnsupportedNodes(formCompatibility.unsupportedNodes) }}
                  </p>
                </div>

                <div v-if="!usesUpstreamRawTemplate" class="stack compact-stack">
                  <div class="selected-node-panel">
                    <div class="selected-node-panel-head">
                      <div>
                        <strong>{{ t('selectedNodes') }}</strong>
                        <span class="hint">
                          {{ t('nodesUnit', { count: selectedNodeCount }) }}
                          <template v-if="missingSelectedNodeCount > 0">{{ t('missingNodes', { count: missingSelectedNodeCount }) }}</template>
                        </span>
                      </div>
                      <button
                        class="button button-ghost button-compact"
                        type="button"
                        :disabled="selectedNodeCount === 0"
                        @click="clearFormNodes"
                      >
                        {{ t('clearSelectedNodes') }}
                      </button>
                    </div>
                    <div v-if="selectedFormNodes.length === 0" class="hint">{{ t('noSelectedNodes') }}</div>
                    <div v-else class="selected-node-chip-list">
                      <span v-for="item in selectedFormNodes" :key="item.id" class="selected-node-chip">
                        <span class="selected-node-chip-main">
                          <strong>{{ item.name }}</strong>
                          <small>{{ item.protocol }} · {{ nodeLatencyText(item) }}</small>
                        </span>
                        <button type="button" :aria-label="t('removeNode')" @click="removeFormNode(item.id)">×</button>
                      </span>
                    </div>
                  </div>

                  <div class="node-picker-toolbar">
                    <div>
                      <label class="field-label" for="subscription-node-group">{{ t('nodeGroupFilter') }}</label>
                      <select id="subscription-node-group" v-model="nodeGroupFilter" class="select">
                        <option value="all">{{ t('allNodeGroups') }}</option>
                        <option value="none">{{ t('ungroupedNodes') }}</option>
                        <option v-for="group in nodeGroups" :key="group.id" :value="group.id">{{ group.name }}</option>
                      </select>
                    </div>
                    <div>
                      <label class="field-label" for="subscription-node-health">{{ t('linkStatus') }}</label>
                      <select id="subscription-node-health" v-model="nodeHealthFilter" class="select">
                        <option value="all">{{ t('showAll') }}</option>
                        <option value="ok">{{ t('showHealthyOnly') }}</option>
                      </select>
                    </div>
                    <div>
                      <label class="field-label" for="subscription-node-search">{{ t('searchNodes') }}</label>
                      <input
                        id="subscription-node-search"
                        v-model.trim="nodeSearch"
                        class="input"
                        type="search"
                        :placeholder="t('searchPlaceholder')"
                      />
                    </div>
                    <button
                      class="button button-ghost button-compact"
                      type="button"
                      :disabled="filteredFormNodes.length === 0"
                      @click="toggleFilteredNodes(true)"
                    >
                      {{ t('selectCurrentGroup') }}
                    </button>
                    <button
                      class="button button-ghost button-compact"
                      type="button"
                      :disabled="filteredFormNodes.length === 0"
                      @click="toggleFilteredNodes(false)"
                    >
                      {{ t('unselectCurrentGroup') }}
                    </button>
                  </div>

                  <div class="field-label">
                    {{ t('selectNodesSummary', { selected: selectedNodeCount, visible: filteredFormNodes.length }) }}
                  </div>
                  <div v-if="nodes.length === 0" class="empty-state">{{ t('noNodesYet') }}</div>
                  <div v-else-if="filteredFormNodes.length === 0" class="empty-state">{{ t('noMatchedNodes') }}</div>
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
              <button class="button button-ghost" type="button" :disabled="saving" @click="closeEditor">{{ t('cancel') }}</button>
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
                {{ t('exportConsoleCopy', { count: exportTargets.length, mode: currentModeLabel }) }}
              </p>
            </div>
            <button class="icon-button" type="button" :aria-label="t('close')" @click="exportConsole = null">x</button>
          </header>

          <div class="export-console-hero">
            <div>
              <div class="hint">{{ t('autoDetectSubscription') }}</div>
              <a class="token-link export-console-auto-link" :href="autoSubscriptionLink(exportConsole)" target="_blank" rel="noreferrer">
                {{ autoSubscriptionLink(exportConsole) }}
              </a>
            </div>
            <button
              class="button button-accent"
              type="button"
              @click="copyText(autoSubscriptionLink(exportConsole), t('autoLinkCopied'))"
            >
              {{ t('copyAutoLink') }}
            </button>
          </div>

          <div class="export-console-grid">
            <article v-for="target in exportTargets" :key="target" class="export-console-card">
              <div class="export-console-card-head">
                <div>
                  <div class="hint">{{ t('client') }}</div>
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
                  @click="copyText(exportLink(exportConsole.token, target), t('targetLinkCopied', { target: TARGET_LABELS[target] }))"
                >
                  {{ t('copyLink') }}
                </button>
                <button
                  v-if="subscriptionCompatibility(exportConsole, target).unsupportedNodes.length > 0"
                  class="button button-ghost button-compact"
                  type="button"
                  @click="openCompatibilityDetail(exportConsole, target)"
                >
                  {{ t('viewWarnings') }}
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
              <span class="eyebrow">{{ t('compatibility') }}</span>
              <h3>{{ compatibilityDetail.title }}</h3>
            </div>
            <button class="icon-button" type="button" :aria-label="t('close')" @click="compatibilityDetail = null">×</button>
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
              <h3>{{ t('subscriptionGroups') }}</h3>
            </div>
            <button class="icon-button" type="button" :aria-label="t('close')" @click="closeGroupEditor">×</button>
          </header>

          <form class="form-grid" @submit.prevent="submitGroup">
            <div>
              <label class="field-label" for="subscription-group-name">{{ t('groupName') }}</label>
              <input id="subscription-group-name" v-model.trim="groupForm.name" class="input" :placeholder="t('subscriptionGroupNamePlaceholder')" />
            </div>
            <div>
              <label class="field-label" for="subscription-group-order">{{ t('sortOrder') }}</label>
              <input id="subscription-group-order" v-model.number="groupForm.sort_order" class="input" type="number" />
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
