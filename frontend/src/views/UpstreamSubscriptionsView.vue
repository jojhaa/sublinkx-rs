<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { extractApiError } from '../api/client'
import { listNodeGroups, type GroupItem } from '../api/groups'
import {
  createUpstreamSubscription,
  deleteUpstreamSubscription,
  importUpstreamSubscription,
  listUpstreamSubscriptions,
  updateUpstreamSubscription,
  type UpstreamSubscriptionItem,
} from '../api/upstreamSubscriptions'
import { useI18n } from '../i18n'

const { t } = useI18n()

const upstreams = ref<UpstreamSubscriptionItem[]>([])
const groups = ref<GroupItem[]>([])
const loading = ref(false)
const saving = ref(false)
const importingId = ref<number | null>(null)
const showEditor = ref(false)
const editingId = ref<number | null>(null)
const errorMessage = ref('')
const successMessage = ref('')

const form = reactive({
  name: '',
  url: '',
  group_id: null as number | null,
  enabled: true,
  remark: '',
})

const isEditing = computed(() => editingId.value !== null)
const submitLabel = computed(() => {
  if (saving.value) {
    return isEditing.value ? t('saving') : t('creating')
  }
  return isEditing.value ? t('saveUpstreamSubscription') : t('createUpstreamSubscription')
})

function resetForm() {
  editingId.value = null
  form.name = ''
  form.url = ''
  form.group_id = null
  form.enabled = true
  form.remark = ''
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

function startEdit(item: UpstreamSubscriptionItem) {
  editingId.value = item.id
  form.name = item.name
  form.url = item.url
  form.group_id = item.group_id
  form.enabled = item.enabled
  form.remark = item.remark
  showEditor.value = true
  errorMessage.value = ''
  successMessage.value = ''
}

function groupName(id: number | null) {
  if (id === null) {
    return t('ungrouped')
  }
  return groups.value.find((item) => item.id === id)?.name ?? t('nodeGroupFallback', { id })
}

function statusLabel(item: UpstreamSubscriptionItem) {
  if (!item.last_import_status) {
    return t('notImported')
  }
  if (item.last_import_status === 'ok') {
    return t('importStatusOk')
  }
  if (item.last_import_status === 'partial') {
    return t('importStatusPartial')
  }
  return t('importStatusError')
}

function statusClass(item: UpstreamSubscriptionItem) {
  if (item.last_import_status === 'ok') {
    return 'status-badge-ok'
  }
  if (item.last_import_status === 'partial') {
    return 'status-badge-warn'
  }
  if (item.last_import_status === 'error') {
    return 'status-badge-warn'
  }
  return 'status-badge-neutral'
}

function formatDate(value: string | null) {
  if (!value) {
    return '-'
  }
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) {
    return value
  }
  return date.toLocaleString()
}

async function load() {
  loading.value = true
  errorMessage.value = ''

  try {
    const [upstreamResponse, groupResponse] = await Promise.all([
      listUpstreamSubscriptions(),
      listNodeGroups(),
    ])
    upstreams.value = upstreamResponse.data
    groups.value = groupResponse.data
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
      url: form.url,
      group_id: form.group_id,
      enabled: form.enabled,
      remark: form.remark || undefined,
    }

    if (editingId.value !== null) {
      await updateUpstreamSubscription(editingId.value, payload)
      successMessage.value = t('upstreamSubscriptionUpdated')
    } else {
      await createUpstreamSubscription(payload)
      successMessage.value = t('upstreamSubscriptionCreated')
    }

    closeEditor()
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    saving.value = false
  }
}

async function runImport(item: UpstreamSubscriptionItem) {
  importingId.value = item.id
  errorMessage.value = ''
  successMessage.value = ''

  try {
    const response = await importUpstreamSubscription(item.id)
    successMessage.value = t('upstreamSubscriptionImported', {
      imported: response.import.imported,
      skipped: response.import.skipped,
      failed: response.import.failed,
    })
    await load()
  } catch (error) {
    errorMessage.value = extractApiError(error)
    await load()
  } finally {
    importingId.value = null
  }
}

async function removeItem(item: UpstreamSubscriptionItem) {
  if (!window.confirm(t('confirmDeleteUpstreamSubscription'))) {
    return
  }

  try {
    await deleteUpstreamSubscription(item.id)
    successMessage.value = t('upstreamSubscriptionDeleted')
    if (editingId.value === item.id) {
      closeEditor()
    }
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
        <span class="eyebrow">Upstream Links</span>
        <h2 class="page-title">{{ t('upstreamSubscriptions') }}</h2>
        <p class="page-copy">{{ t('upstreamSubscriptionsCopy') }}</p>
      </div>
      <div class="inline-actions">
        <button class="button button-ghost" type="button" :disabled="loading" @click="load">
          {{ loading ? t('refreshing') : t('refresh') }}
        </button>
        <button class="button button-accent" type="button" @click="openCreate">
          {{ t('createUpstreamSubscription') }}
        </button>
      </div>
    </header>

    <div v-if="errorMessage" class="error-banner">{{ errorMessage }}</div>
    <div v-if="successMessage" class="success-banner">{{ successMessage }}</div>

    <article class="card stack management-card">
      <div class="section-bar">
        <div>
          <div class="hint">{{ t('upstreamSubscriptionList') }}</div>
          <p class="card-copy">{{ t('upstreamSubscriptionListCopy') }}</p>
        </div>
      </div>

      <div v-if="upstreams.length === 0" class="empty-state">{{ t('emptyUpstreamSubscriptions') }}</div>

      <div v-else class="table-wrap">
        <table class="table dense-table subscription-table">
          <thead>
            <tr>
              <th>{{ t('upstreamSubscription') }}</th>
              <th>{{ t('importTarget') }}</th>
              <th>{{ t('lastImport') }}</th>
              <th>{{ t('actions') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="item in upstreams" :key="item.id">
              <td>
                <div class="subscription-title-row">
                  <strong class="row-title">{{ item.name }}</strong>
                  <span class="status-badge" :class="item.enabled ? 'status-badge-ok' : 'status-badge-muted'">
                    {{ item.enabled ? t('enabled') : t('disabled') }}
                  </span>
                </div>
                <div class="row-meta upstream-url">{{ item.url }}</div>
                <div v-if="item.remark" class="row-meta">{{ item.remark }}</div>
              </td>
              <td>
                <span class="status-badge status-badge-neutral">{{ groupName(item.group_id) }}</span>
                <div v-if="item.template_name" class="row-meta">{{ item.template_name }}</div>
              </td>
              <td>
                <span class="status-badge" :class="statusClass(item)">{{ statusLabel(item) }}</span>
                <div class="row-meta">{{ formatDate(item.last_imported_at) }}</div>
                <div class="row-meta">
                  {{ t('importStats', {
                    imported: item.last_import_imported,
                    skipped: item.last_import_skipped,
                    failed: item.last_import_failed,
                  }) }}
                </div>
                <div v-if="item.last_import_message" class="row-meta">{{ item.last_import_message }}</div>
              </td>
              <td>
                <div class="inline-actions row-actions">
                  <button
                    class="button button-accent button-compact"
                    type="button"
                    :disabled="importingId === item.id || !item.enabled"
                    @click="runImport(item)"
                  >
                    {{ importingId === item.id ? t('importingNode') : t('importNow') }}
                  </button>
                  <button class="button button-ghost button-compact" type="button" @click="startEdit(item)">
                    {{ t('edit') }}
                  </button>
                  <button class="button button-danger button-compact" type="button" @click="removeItem(item)">
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
              <span class="eyebrow">{{ isEditing ? 'Edit Upstream' : 'New Upstream' }}</span>
              <h3>{{ isEditing ? t('editUpstreamSubscription') : t('createUpstreamSubscription') }}</h3>
            </div>
            <button class="icon-button" type="button" :aria-label="t('close')" @click="closeEditor">x</button>
          </header>

          <form class="form-grid" @submit.prevent="submit">
            <div>
              <label class="field-label" for="upstream-name">{{ t('upstreamSubscriptionName') }}</label>
              <input id="upstream-name" v-model.trim="form.name" class="input" :placeholder="t('upstreamSubscriptionNamePlaceholder')" />
            </div>

            <div>
              <label class="field-label" for="upstream-url-managed">{{ t('upstreamUrl') }}</label>
              <input
                id="upstream-url-managed"
                v-model.trim="form.url"
                class="input"
                type="url"
                :placeholder="t('upstreamUrlPlaceholder')"
              />
              <div class="hint template-kind-hint">{{ t('upstreamSubscriptionSaveHint') }}</div>
            </div>

            <div>
              <label class="field-label" for="upstream-group-managed">{{ t('importToNodeGroup') }}</label>
              <select id="upstream-group-managed" v-model="form.group_id" class="select">
                <option :value="null">{{ t('ungrouped') }}</option>
                <option v-for="group in groups" :key="group.id" :value="group.id">
                  {{ group.name }}
                </option>
              </select>
            </div>

            <div>
              <label class="field-label" for="upstream-remark-managed">{{ t('remark') }}</label>
              <input id="upstream-remark-managed" v-model.trim="form.remark" class="input" :placeholder="t('remarkPlaceholder')" />
            </div>

            <label class="toggle-row">
              <input v-model="form.enabled" type="checkbox" />
              <span>{{ t('enableUpstreamSubscription') }}</span>
            </label>

            <div class="modal-actions">
              <button class="button button-ghost" type="button" :disabled="saving" @click="closeEditor">{{ t('cancel') }}</button>
              <button class="button button-accent" type="submit" :disabled="saving || !form.name || !form.url">
                {{ submitLabel }}
              </button>
            </div>
          </form>
        </section>
      </div>
    </Teleport>
  </section>
</template>
