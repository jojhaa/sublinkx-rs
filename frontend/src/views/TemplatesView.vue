<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { extractApiError } from '../api/client'
import { useI18n, type MessageKey } from '../i18n'
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

const templates = ref<TemplateItem[]>([])
const loading = ref(false)
const saving = ref(false)
const showEditor = ref(false)
const editingId = ref<number | null>(null)
const errorMessage = ref('')
const successMessage = ref('')

const form = reactive({
  name: '',
  kind: 'mihomo' as TemplateKind,
  content: '',
})

const isEditing = computed(() => editingId.value !== null)
const selectedKindNote = computed(
  () => t(TEMPLATE_KIND_OPTIONS.find((item) => item.value === form.kind)?.noteKey ?? 'templateKindCommonNote'),
)
const submitLabel = computed(() => {
  if (saving.value) {
    return isEditing.value ? t('saving') : t('creating')
  }

  return isEditing.value ? t('saveTemplate') : t('createTemplate')
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
  return templates.value.filter((item) => item.kind === kind).length
}

async function load() {
  loading.value = true
  errorMessage.value = ''

  try {
    const response = await listTemplates()
    templates.value = response.data
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
  if (!window.confirm(t('confirmDeleteTemplate'))) {
    return
  }

  try {
    await deleteTemplate(id)

    if (editingId.value === id) {
      closeEditor()
    }

    successMessage.value = t('templateDeleted')
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

    <article class="card stack template-type-panel">
      <div class="section-bar">
        <div>
          <div class="hint">{{ t('supportedTemplateTypes') }}</div>
          <p class="card-copy">{{ t('supportedTemplateTypesCopy') }}</p>
        </div>
      </div>

      <div class="target-badge-grid template-kind-grid">
        <button
          v-for="option in TEMPLATE_KIND_OPTIONS"
          :key="option.value"
          class="status-badge status-badge-neutral"
          type="button"
          @click="openCreate(option.value)"
        >
          {{ option.label }}
          <span class="muted">({{ templateCountByKind(option.value) }})</span>
        </button>
      </div>
    </article>

    <article class="card stack management-card">
      <div class="section-bar">
        <div>
          <div class="hint">{{ t('templateList') }}</div>
          <p class="card-copy">{{ t('templateListCopy') }}</p>
        </div>
      </div>

      <div v-if="templates.length === 0" class="empty-state">{{ t('emptyTemplates') }}</div>

      <div v-else class="table-wrap">
        <table class="table dense-table template-table">
          <thead>
            <tr>
              <th>{{ t('template') }}</th>
              <th>{{ t('content') }}</th>
              <th>{{ t('actions') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="item in templates" :key="item.id">
              <td>
                <div class="subscription-title-row">
                  <strong class="row-title">{{ item.name }}</strong>
                  <span class="status-badge status-badge-neutral">{{ kindLabel(item.kind) }}</span>
                </div>
                <div class="row-meta">#{{ item.id }}</div>
              </td>
              <td>
                <code class="code-block-preview compact-code-preview">{{ item.content }}</code>
              </td>
              <td>
                <div class="inline-actions row-actions">
                  <button class="button button-ghost button-compact" type="button" @click="startEdit(item)">
                    {{ t('edit') }}
                  </button>
                  <button class="button button-danger button-compact" type="button" @click="removeTemplate(item.id)">
                    {{ t('delete') }}
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </article>

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
