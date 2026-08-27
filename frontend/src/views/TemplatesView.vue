<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { extractApiError } from '../api/client'
import { useI18n, type MessageKey } from '../i18n'
import { readStoredPageSize, storePageSize } from '../utils/pagination'
import {
  createTemplate,
  deleteTemplate,
  listTemplates,
  updateTemplate,
  type TemplateItem,
} from '../api/templates'

const { t } = useI18n()

type TemplateKind =
  | 'common'
  | 'clash'
  | 'mihomo'
  | 'xray'
  | 'surge'
  | 'sing-box'
  | 'surge2'
  | 'surge3'
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

interface TemplateKindOption {
  value: TemplateKind
  label: string
  noteKey: MessageKey
}

const TEMPLATE_KIND_OPTIONS: TemplateKindOption[] = [
  { value: 'common', label: 'Common', noteKey: 'templateKindCommonNote' },
  { value: 'clash', label: 'Clash', noteKey: 'templateKindClashNote' },
  { value: 'mihomo', label: 'Mihomo', noteKey: 'templateKindMihomoNote' },
  { value: 'xray', label: 'Xray / V2Ray', noteKey: 'templateKindXrayNote' },
  { value: 'surge', label: 'Surge 4/5', noteKey: 'templateKindSurgeNote' },
  { value: 'sing-box', label: 'sing-box', noteKey: 'templateKindSingBoxNote' },
  { value: 'surge3', label: 'Surge 3', noteKey: 'templateKindSurge3Note' },
  { value: 'surge2', label: 'Surge 2', noteKey: 'templateKindSurge2Note' },
  { value: 'quanx', label: 'Quantumult X', noteKey: 'templateKindQuanxNote' },
  { value: 'quan', label: 'Quantumult', noteKey: 'templateKindQuanNote' },
  { value: 'loon', label: 'Loon', noteKey: 'templateKindLoonNote' },
  { value: 'surfboard', label: 'Surfboard', noteKey: 'templateKindSurfboardNote' },
  { value: 'mellow', label: 'Mellow', noteKey: 'templateKindMellowNote' },
  { value: 'clashr', label: 'ClashR', noteKey: 'templateKindClashrNote' },
  { value: 'ss', label: 'SS SIP002', noteKey: 'templateKindSsNote' },
  { value: 'sssub', label: 'SS SIP008', noteKey: 'templateKindSssubNote' },
  { value: 'ssr', label: 'ShadowsocksR', noteKey: 'templateKindSsrNote' },
  { value: 'ssd', label: 'ShadowsocksD', noteKey: 'templateKindSsdNote' },
  { value: 'trojan', label: 'Trojan URI', noteKey: 'templateKindTrojanNote' },
  { value: 'mixed', label: 'Mixed', noteKey: 'templateKindMixedNote' },
]

const PAGE_SIZE_OPTIONS = [12, 24, 48]
const PAGE_SIZE_STORAGE_KEY = 'sublinkx_templates_page_size'

const templates = ref<TemplateItem[]>([])
const loading = ref(false)
const saving = ref(false)
const showEditor = ref(false)
const editingId = ref<number | null>(null)
const selectedIds = ref<number[]>([])
const kindFilter = ref<TemplateKind | 'all'>('all')
const page = ref(1)
const pageSize = ref(readStoredPageSize(PAGE_SIZE_STORAGE_KEY, PAGE_SIZE_OPTIONS, 12))
const totalTemplates = ref(0)
const totalPages = ref(0)
const templateKindCounts = ref<Record<string, number>>({})
const errorMessage = ref('')
const successMessage = ref('')

const form = reactive({
  name: '',
  kind: 'mihomo' as TemplateKind,
  content: '',
})

const isEditing = computed(() => editingId.value !== null)
const filteredTemplates = computed(() => templates.value)
const pageCount = computed(() => Math.max(1, totalPages.value))
const overallTemplateCount = computed(() => Object.values(templateKindCounts.value).reduce((sum, count) => sum + count, 0))
const pagedTemplates = computed(() => templates.value)
const currentPageCustomTemplates = computed(() => pagedTemplates.value.filter((item) => !item.is_builtin))
const selectedTemplates = computed(() => templates.value.filter((item) => selectedIds.value.includes(item.id)))
const selectedCount = computed(() => selectedTemplates.value.length)
const allCustomTemplatesSelected = computed(
  () =>
    currentPageCustomTemplates.value.length > 0 &&
    currentPageCustomTemplates.value.every((item) => selectedIds.value.includes(item.id)),
)
const selectedKindNote = computed(
  () => t(TEMPLATE_KIND_OPTIONS.find((item) => item.value === form.kind)?.noteKey ?? 'templateKindCommonNote'),
)
const submitLabel = computed(() => {
  if (saving.value) {
    return isEditing.value ? t('saving') : t('creating')
  }

  return isEditing.value ? t('saveTemplate') : t('createTemplate')
})

watch(pageSize, (value) => {
  storePageSize(PAGE_SIZE_STORAGE_KEY, value)
  reloadFirstTemplatePage()
})

watch(page, () => {
  selectedIds.value = []
  void load()
})

function normalizeKind(kind: string): TemplateKind {
  return TEMPLATE_KIND_OPTIONS.some((item) => item.value === kind) ? (kind as TemplateKind) : 'common'
}

function resetForm() {
  editingId.value = null
  form.name = ''
  form.kind = 'mihomo'
  form.content = ''
}

function openCreate(kind?: TemplateKind) {
  resetForm()
  if (kind) {
    form.kind = kind
  }
  showEditor.value = true
  errorMessage.value = ''
  successMessage.value = ''
}

function closeEditor() {
  showEditor.value = false
  resetForm()
}

function startEdit(item: TemplateItem) {
  editingId.value = item.id
  form.name = item.name
  form.kind = normalizeKind(item.kind)
  form.content = item.content
  showEditor.value = true
  errorMessage.value = ''
  successMessage.value = ''
}

function kindLabel(kind: string) {
  return TEMPLATE_KIND_OPTIONS.find((item) => item.value === kind)?.label ?? kind
}

function templateCountByKind(kind: TemplateKind) {
  return templateKindCounts.value[kind] ?? 0
}

function setKindFilter(kind: TemplateKind | 'all') {
  kindFilter.value = kind
  reloadFirstTemplatePage()
  selectedIds.value = selectedIds.value.filter((id) =>
    templates.value.some((item) => item.id === id && !item.is_builtin && (kind === 'all' || item.kind === kind)),
  )
}

function reloadFirstTemplatePage() {
  if (page.value === 1) {
    void load()
  } else {
    page.value = 1
  }
}

function selectAllCustomTemplates(checked: boolean) {
  if (!checked) {
    const visibleIds = new Set(currentPageCustomTemplates.value.map((item) => item.id))
    selectedIds.value = selectedIds.value.filter((id) => !visibleIds.has(id))
    return
  }

  selectedIds.value = Array.from(new Set([...selectedIds.value, ...currentPageCustomTemplates.value.map((item) => item.id)]))
}

function toggleTemplateSelection(id: number, checked: boolean) {
  if (checked) {
    selectedIds.value = Array.from(new Set([...selectedIds.value, id]))
    return
  }

  selectedIds.value = selectedIds.value.filter((selectedId) => selectedId !== id)
}

function clearSelection() {
  selectedIds.value = []
}

async function load() {
  loading.value = true
  errorMessage.value = ''

  try {
    const response = await listTemplates({
      page: page.value,
      page_size: pageSize.value,
      ...(kindFilter.value === 'all' ? {} : { kind: kindFilter.value }),
    })
    templates.value = response.data
    totalTemplates.value = response.pagination.total
    totalPages.value = response.pagination.total_pages
    templateKindCounts.value = Object.fromEntries(response.kind_counts.map((item) => [item.kind, item.count]))
    selectedIds.value = selectedIds.value.filter((id) =>
      response.data.some((item) => item.id === id && !item.is_builtin),
    )
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
      kind: form.kind,
      content: form.content,
    }

    if (editingId.value !== null) {
      await updateTemplate(editingId.value, payload)
      successMessage.value = t('templateUpdated')
    } else {
      await createTemplate(payload)
      successMessage.value = t('templateCreated')
    }

    closeEditor()
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    saving.value = false
  }
}

async function removeTemplate(id: number) {
  const item = templates.value.find((template) => template.id === id)
  if (item?.is_builtin) {
    errorMessage.value = t('builtinTemplateDeleteBlocked')
    return
  }

  if (!window.confirm(t('confirmDeleteTemplate'))) {
    return
  }

  try {
    await deleteTemplate(id)

    if (editingId.value === id) {
      closeEditor()
    }

    selectedIds.value = selectedIds.value.filter((selectedId) => selectedId !== id)
    successMessage.value = t('templateDeleted')
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  }
}

async function removeSelectedTemplates() {
  if (selectedTemplates.value.length === 0) {
    return
  }

  const ids = selectedTemplates.value.filter((item) => !item.is_builtin).map((item) => item.id)
  if (ids.length === 0) {
    errorMessage.value = t('noDeletableTemplatesSelected')
    return
  }

  if (!window.confirm(t('confirmDeleteSelectedTemplates', { count: ids.length }))) {
    return
  }

  saving.value = true
  errorMessage.value = ''
  successMessage.value = ''

  try {
    const results = await Promise.allSettled(ids.map((id) => deleteTemplate(id)))
    const successCount = results.filter((item) => item.status === 'fulfilled').length
    const failures = results.filter((item): item is PromiseRejectedResult => item.status === 'rejected')

    if (successCount === 0) {
      throw failures[0]?.reason ?? new Error(t('batchDeleteFailed'))
    }

    if (editingId.value !== null && ids.includes(editingId.value)) {
      closeEditor()
    }

    selectedIds.value = selectedIds.value.filter((id) => !ids.includes(id))
    successMessage.value = t('templatesDeleted', { count: successCount })
    await load()

    if (failures.length > 0) {
      errorMessage.value = t('templateDeleteFailures', { count: failures.length, reason: extractApiError(failures[0].reason) })
    }
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
        <span class="eyebrow">Templates</span>
        <h2 class="page-title">{{ t('templates') }}</h2>
        <p class="page-copy">{{ t('templatesCopy') }}</p>
      </div>
      <div class="inline-actions">
        <button class="button button-ghost" type="button" :disabled="loading" @click="load">
          {{ loading ? t('refreshing') : t('refresh') }}
        </button>
        <button class="button button-accent" type="button" @click="openCreate()">{{ t('createTemplate') }}</button>
      </div>
    </header>

    <div v-if="errorMessage" class="error-banner">{{ errorMessage }}</div>
    <div v-if="successMessage" class="success-banner">{{ successMessage }}</div>

    <section class="template-console">
      <aside class="card template-type-panel template-rail">
        <div class="template-rail-header">
          <div>
            <div class="hint">{{ t('supportedTemplateTypes') }}</div>
            <p class="card-copy">{{ t('supportedTemplateTypesCopy') }}</p>
          </div>
          <button class="button button-ghost button-compact" type="button" @click="openCreate(normalizeKind(kindFilter === 'all' ? 'mihomo' : kindFilter))">
            {{ t('createTemplate') }}
          </button>
        </div>

        <div class="template-kind-list">
          <button
            class="template-kind-option"
            :class="{ active: kindFilter === 'all' }"
            type="button"
            @click="setKindFilter('all')"
          >
            <span>{{ t('allTemplateKinds') }}</span>
            <strong>{{ overallTemplateCount }}</strong>
          </button>
          <button
            v-for="option in TEMPLATE_KIND_OPTIONS"
            :key="option.value"
            class="template-kind-option"
            :class="{ active: kindFilter === option.value }"
            type="button"
            @click="setKindFilter(option.value)"
          >
            <span>{{ option.label }}</span>
            <strong>{{ templateCountByKind(option.value) }}</strong>
          </button>
        </div>
      </aside>

      <article class="card stack management-card template-list-panel">
        <div class="section-bar template-list-bar">
          <div>
            <div class="hint">{{ t('templateList') }}</div>
            <p class="card-copy">{{ t('templateListSummary', { count: totalTemplates, selected: selectedCount }) }}</p>
          </div>
          <div class="inline-actions bulk-actions" :class="{ 'is-empty-selection': selectedCount === 0 }">
            <button class="button button-ghost" type="button" :disabled="selectedCount === 0" @click="clearSelection">{{ t('clearSelection') }}</button>
            <button class="button button-danger" type="button" :disabled="saving || selectedCount === 0" @click="removeSelectedTemplates">
              {{ t('deleteSelected') }}
            </button>
          </div>
        </div>

        <div v-if="templates.length === 0" class="empty-state">{{ t('emptyTemplates') }}</div>
        <div v-else-if="filteredTemplates.length === 0" class="empty-state">{{ t('emptyFilteredTemplates') }}</div>

        <div v-else class="template-list">
          <div class="template-list-head">
            <label class="template-select-all">
              <input
                type="checkbox"
                :checked="allCustomTemplatesSelected"
                :disabled="currentPageCustomTemplates.length === 0"
                :aria-label="allCustomTemplatesSelected ? t('unselectCurrentPage') : t('selectCurrentPage')"
                @change="selectAllCustomTemplates(($event.target as HTMLInputElement).checked)"
              />
              <span>{{ t('selectCurrentPage') }}</span>
            </label>
            <span class="muted">{{ t('templateListCopy') }}</span>
          </div>

          <div class="template-card-grid">
            <article
              v-for="item in pagedTemplates"
              :key="item.id"
              class="template-card"
              :class="{ 'is-selected': selectedIds.includes(item.id), 'is-builtin': item.is_builtin }"
            >
              <div class="template-card-head">
                <label class="template-card-check">
                  <input
                    type="checkbox"
                    :checked="selectedIds.includes(item.id)"
                    :disabled="item.is_builtin"
                    :aria-label="item.is_builtin ? t('builtinTemplate') : t('selectTemplate')"
                    @change="toggleTemplateSelection(item.id, ($event.target as HTMLInputElement).checked)"
                  />
                </label>

                <div class="template-title-stack">
                  <strong class="row-title template-card-title">{{ item.name }}</strong>
                  <div class="row-meta">#{{ item.id }} · {{ kindLabel(item.kind) }}</div>
                </div>

                <span class="status-badge" :class="item.is_builtin ? 'status-badge-neutral' : 'status-badge-ok'">
                  {{ item.is_builtin ? t('builtinTemplate') : t('customTemplate') }}
                </span>
              </div>

              <div class="template-card-meta">
                <span class="status-badge status-badge-neutral">{{ item.kind }}</span>
                <span class="muted">{{ t('templateContent') }}</span>
              </div>

              <code class="template-preview-line">{{ item.content }}</code>

              <div class="template-card-actions">
                <button class="button button-ghost button-compact" type="button" @click="startEdit(item)">
                  {{ t('edit') }}
                </button>
                <button class="button button-danger button-compact" type="button" :disabled="item.is_builtin" @click="removeTemplate(item.id)">
                  {{ t('delete') }}
                </button>
              </div>
            </article>
          </div>

          <footer class="pagination-bar template-pagination-bar">
            <span class="hint">{{ t('pageLabel', { page, count: pageCount }) }}</span>
            <select v-model.number="pageSize" class="select page-size-select" :aria-label="t('pageSize', { size: pageSize })">
              <option v-for="size in PAGE_SIZE_OPTIONS" :key="size" :value="size">{{ t('pageSize', { size }) }}</option>
            </select>
            <div class="inline-actions">
              <button class="button button-ghost button-compact" type="button" :disabled="page <= 1" @click="page -= 1">{{ t('previousPage') }}</button>
              <button class="button button-ghost button-compact" type="button" :disabled="page >= pageCount" @click="page += 1">{{ t('nextPage') }}</button>
            </div>
          </footer>
        </div>
      </article>
    </section>

    <Teleport to="body">
      <div v-if="showEditor" class="modal-backdrop" @click.self="closeEditor">
        <section class="modal-panel">
          <header class="modal-header">
            <div>
              <span class="eyebrow">{{ isEditing ? 'Edit Template' : 'New Template' }}</span>
              <h3>{{ isEditing ? t('editTemplate') : t('createTemplate') }}</h3>
            </div>
            <button class="icon-button" type="button" :aria-label="t('close')" @click="closeEditor">x</button>
          </header>

          <form class="form-grid" @submit.prevent="submit">
            <div>
              <label class="field-label" for="template-name">{{ t('templateName') }}</label>
              <input id="template-name" v-model.trim="form.name" class="input" :placeholder="t('templateNamePlaceholder')" />
            </div>

            <div>
              <label class="field-label" for="template-kind">{{ t('templateType') }}</label>
              <select id="template-kind" v-model="form.kind" class="select">
                <option v-for="option in TEMPLATE_KIND_OPTIONS" :key="option.value" :value="option.value">
                  {{ option.label }} - {{ t('templateReady') }}
                </option>
              </select>
              <div class="hint template-kind-hint">{{ selectedKindNote }}</div>
            </div>

            <div>
              <label class="field-label" for="template-content">{{ t('templateContent') }}</label>
              <textarea
                id="template-content"
                v-model.trim="form.content"
                class="textarea code-textarea"
                :placeholder="t('templateContentPlaceholder')"
              />
            </div>

            <div class="modal-actions">
              <button class="button button-ghost" type="button" :disabled="saving" @click="closeEditor">{{ t('cancel') }}</button>
              <button class="button button-accent" type="submit" :disabled="saving || !form.name || !form.content">
                {{ submitLabel }}
              </button>
            </div>
          </form>
        </section>
      </div>
    </Teleport>
  </section>
</template>
