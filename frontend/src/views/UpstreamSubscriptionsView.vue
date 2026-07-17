<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { extractApiError } from '../api/client'
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
  enabled: true,
  sync_enabled: false,
  sync_interval_minutes: 360,
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
  form.enabled = true
  form.sync_enabled = false
  form.sync_interval_minutes = 360
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
  form.enabled = item.enabled
  form.sync_enabled = item.sync_enabled
  form.sync_interval_minutes = item.sync_interval_minutes
  form.remark = item.remark
  showEditor.value = true
  errorMessage.value = ''
  successMessage.value = ''
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

function urlHost(value: string) {
  try {
    return new URL(value).host || value
  } catch {
    return value.replace(/^https?:\/\//, '').split('/')[0] || value
  }
}

function compactUrl(value: string) {
  const stripped = value.replace(/^https?:\/\//, '')
  if (stripped.length <= 88) {
    return stripped
  }
  return `${stripped.slice(0, 54)}...${stripped.slice(-20)}`
}

async function load() {
  loading.value = true
  errorMessage.value = ''

  try {
    const upstreamResponse = await listUpstreamSubscriptions()
    upstreams.value = upstreamResponse.data
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
      enabled: form.enabled,
      sync_enabled: form.sync_enabled,
      sync_interval_minutes: form.sync_interval_minutes,
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
      updated: response.import.updated,
      disabled: response.import.disabled,
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
  const deleteNodes = window.confirm(t('confirmDeleteUpstreamSubscriptionNodes'))

  try {
    const response = await deleteUpstreamSubscription(item.id, deleteNodes)
    successMessage.value = deleteNodes
      ? t('upstreamSubscriptionDeletedWithNodes', { count: response.deleted_nodes })
      : t('upstreamSubscriptionDeleted')
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

      <div v-else class="upstream-card-list">
        <article v-for="item in upstreams" :key="item.id" class="upstream-card" :class="{ 'is-disabled': !item.enabled }">
          <div class="upstream-card-main">
            <div class="upstream-card-kicker">
              <span class="status-badge" :class="item.enabled ? 'status-badge-ok' : 'status-badge-muted'">
                {{ item.enabled ? t('enabled') : t('disabled') }}
              </span>
              <span class="status-badge" :class="item.sync_enabled ? 'status-badge-ok' : 'status-badge-neutral'">
                {{ item.sync_enabled ? t('upstreamSyncEvery', { minutes: item.sync_interval_minutes }) : t('upstreamSyncOff') }}
              </span>
              <span class="status-badge status-badge-neutral">{{ t('upstreamAutoGroupBadge', { name: item.name }) }}</span>
              <span v-if="item.template_name" class="status-badge status-badge-neutral">{{ item.template_name }}</span>
            </div>
            <h3 class="upstream-card-title">{{ item.name }}</h3>
            <div class="upstream-url-panel" :title="item.url">
              <span class="upstream-url-host">{{ urlHost(item.url) }}</span>
              <span class="upstream-url-short">{{ compactUrl(item.url) }}</span>
            </div>
            <p v-if="item.remark" class="upstream-card-note">{{ item.remark }}</p>
          </div>

          <div class="upstream-card-side">
            <div class="upstream-import-panel">
              <div class="upstream-import-head">
                <span class="status-badge" :class="statusClass(item)">{{ statusLabel(item) }}</span>
                <span class="upstream-import-time">{{ formatDate(item.last_imported_at) }}</span>
              </div>
              <div class="upstream-import-stats">
                {{ t('importStats', {
                  imported: item.last_import_imported,
                  updated: item.last_import_updated,
                  disabled: item.last_import_disabled,
                  skipped: item.last_import_skipped,
                  failed: item.last_import_failed,
                }) }}
              </div>
              <div v-if="item.last_import_message" class="upstream-import-message">{{ item.last_import_message }}</div>
            </div>

            <div class="inline-actions row-actions upstream-card-actions">
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
          </div>
        </article>
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

            <div class="upstream-auto-group-panel">
              <span class="status-badge status-badge-neutral">{{ t('importTarget') }}</span>
              <strong>{{ form.name || t('upstreamSubscriptionName') }}</strong>
              <p>{{ t('upstreamAutoGroupHint') }}</p>
            </div>

            <div>
              <label class="field-label" for="upstream-remark-managed">{{ t('remark') }}</label>
              <input id="upstream-remark-managed" v-model.trim="form.remark" class="input" :placeholder="t('remarkPlaceholder')" />
            </div>

            <label class="toggle-row">
              <input v-model="form.enabled" type="checkbox" />
              <span>{{ t('enableUpstreamSubscription') }}</span>
            </label>

            <label class="toggle-row">
              <input v-model="form.sync_enabled" type="checkbox" />
              <span>{{ t('enableUpstreamAutoSync') }}</span>
            </label>

            <div>
              <label class="field-label" for="upstream-sync-interval">{{ t('upstreamSyncInterval') }}</label>
              <input
                id="upstream-sync-interval"
                v-model.number="form.sync_interval_minutes"
                class="input"
                type="number"
                min="5"
                max="10080"
              />
              <div class="hint template-kind-hint">{{ t('upstreamSyncHint') }}</div>
            </div>

            <div class="modal-actions">
              <button class="button button-ghost" type="button" :disabled="saving" @click="closeEditor">{{ t('cancel') }}</button>
              <button
                class="button button-accent"
                type="submit"
                :disabled="saving || !form.name || !form.url || form.sync_interval_minutes < 5"
              >
                {{ submitLabel }}
              </button>
            </div>
          </form>
        </section>
      </div>
    </Teleport>
  </section>
</template>
