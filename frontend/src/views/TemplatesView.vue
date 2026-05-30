<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { extractApiError } from '../api/client'
import {
  createTemplate,
  deleteTemplate,
  listTemplates,
  updateTemplate,
  type TemplateItem,
} from '../api/templates'

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
  note: string
  status: '已可导出'
}

const TEMPLATE_KIND_OPTIONS: TemplateKindOption[] = [
  { value: 'common', label: 'Common', note: '通用片段，适合跨客户端复用。', status: '已可导出' },
  { value: 'clash', label: 'Clash', note: 'Clash 原生 YAML 模板，默认走 Mihomo 渲染器。', status: '已可导出' },
  { value: 'mihomo', label: 'Mihomo', note: 'Mihomo / Clash Verge Rev / Stash 方向。', status: '已可导出' },
  { value: 'xray', label: 'Xray / V2Ray', note: 'URI bundle 类型，适合 v2rayN / v2rayNG。', status: '已可导出' },
  { value: 'surge', label: 'Surge 4/5', note: 'Surge INI 模板。', status: '已可导出' },
  { value: 'sing-box', label: 'sing-box', note: 'sing-box JSON 模板，适合 NekoBox / Hiddify。', status: '已可导出' },
  { value: 'surge3', label: 'Surge 3', note: '旧版 Surge 兼容模板，复用 Surge 渲染器。', status: '已可导出' },
  { value: 'surge2', label: 'Surge 2', note: '老版 Surge 兼容模板，复用 Surge 渲染器。', status: '已可导出' },
  { value: 'quanx', label: 'Quantumult X', note: 'QuanX 配置文本导出。', status: '已可导出' },
  { value: 'quan', label: 'Quantumult', note: 'Quantumult 兼容配置文本导出。', status: '已可导出' },
  { value: 'loon', label: 'Loon', note: 'Loon 兼容配置文本导出。', status: '已可导出' },
  { value: 'surfboard', label: 'Surfboard', note: 'Surfboard 兼容配置文本导出。', status: '已可导出' },
  { value: 'mellow', label: 'Mellow', note: 'Mellow 使用 Clash/Mihomo YAML 兼容导出。', status: '已可导出' },
  { value: 'clashr', label: 'ClashR', note: 'ClashR 使用 Clash/Mihomo YAML 兼容导出。', status: '已可导出' },
  { value: 'ss', label: 'SS SIP002', note: 'Shadowsocks SIP002 URI bundle 导出。', status: '已可导出' },
  { value: 'sssub', label: 'SS SIP008', note: 'Shadowsocks Android SIP008 JSON 导出。', status: '已可导出' },
  { value: 'ssr', label: 'ShadowsocksR', note: 'SSR URI bundle 导出。', status: '已可导出' },
  { value: 'ssd', label: 'ShadowsocksD', note: 'SSD JSON 订阅导出。', status: '已可导出' },
  { value: 'trojan', label: 'Trojan URI', note: 'Trojan URI bundle 导出。', status: '已可导出' },
  { value: 'mixed', label: 'Mixed', note: '混合 URI bundle 导出。', status: '已可导出' },
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
const templateCount = computed(() => templates.value.length)
const createdKindCount = computed(() => new Set(templates.value.map((item) => item.kind)).size)
const supportedKindCount = computed(() => TEMPLATE_KIND_OPTIONS.length)
const selectedKindNote = computed(
  () => TEMPLATE_KIND_OPTIONS.find((item) => item.value === form.kind)?.note ?? '',
)
const submitLabel = computed(() => {
  if (saving.value) {
    return isEditing.value ? '保存中...' : '创建中...'
  }

  return isEditing.value ? '保存模板' : '创建模板'
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
      successMessage.value = '模板已更新。'
    } else {
      await createTemplate(payload)
      successMessage.value = '模板已创建。'
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
  if (!window.confirm('确定删除这个模板吗？')) {
    return
  }

  try {
    await deleteTemplate(id)

    if (editingId.value === id) {
      closeEditor()
    }

    successMessage.value = '模板已删除。'
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
        <h2 class="page-title">模板管理</h2>
        <p class="page-copy">
          已创建 {{ templateCount }} 个模板，覆盖 {{ createdKindCount }} 种类型。系统已支持
          {{ supportedKindCount }} 种模板类型。
        </p>
      </div>
      <div class="inline-actions">
        <button class="button button-ghost" type="button" :disabled="loading" @click="load">
          {{ loading ? '刷新中...' : '刷新' }}
        </button>
        <button class="button button-accent" type="button" @click="openCreate()">新建模板</button>
      </div>
    </header>

    <div v-if="errorMessage" class="error-banner">{{ errorMessage }}</div>
    <div v-if="successMessage" class="success-banner">{{ successMessage }}</div>

    <article class="card stack template-type-panel">
      <div class="section-bar">
        <div>
          <div class="hint">支持的模板类型</div>
          <p class="card-copy">这里展示系统支持的客户端模板类型；下面列表才是数据库里已创建的模板。</p>
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
          <div class="hint">模板列表</div>
          <p class="card-copy">模板内容只保留短预览，编辑放到弹窗里处理。</p>
        </div>
      </div>

      <div v-if="templates.length === 0" class="empty-state">还没有模板，先新建一个作为基线。</div>

      <div v-else class="table-wrap">
        <table class="table dense-table template-table">
          <thead>
            <tr>
              <th>模板</th>
              <th>内容</th>
              <th>操作</th>
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
                    编辑
                  </button>
                  <button class="button button-danger button-compact" type="button" @click="removeTemplate(item.id)">
                    删除
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
              <h3>{{ isEditing ? '编辑模板' : '新建模板' }}</h3>
            </div>
            <button class="icon-button" type="button" aria-label="关闭" @click="closeEditor">x</button>
          </header>

          <form class="form-grid" @submit.prevent="submit">
            <div>
              <label class="field-label" for="template-name">模板名称</label>
              <input id="template-name" v-model.trim="form.name" class="input" placeholder="例如：Mihomo Mobile Base" />
            </div>

            <div>
              <label class="field-label" for="template-kind">模板类型</label>
              <select id="template-kind" v-model="form.kind" class="select">
                <option v-for="option in TEMPLATE_KIND_OPTIONS" :key="option.value" :value="option.value">
                  {{ option.label }} - {{ option.status }}
                </option>
              </select>
              <div class="hint template-kind-hint">{{ selectedKindNote }}</div>
            </div>

            <div>
              <label class="field-label" for="template-content">模板内容</label>
              <textarea
                id="template-content"
                v-model.trim="form.content"
                class="textarea code-textarea"
                placeholder="写入 YAML、JSON、INI 或对应客户端配置片段"
              />
            </div>

            <div class="modal-actions">
              <button class="button button-ghost" type="button" :disabled="saving" @click="closeEditor">取消</button>
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
