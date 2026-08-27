import apiClient, { type PaginationMeta } from './client'
import { readAuthToken, readCsrfToken } from '../utils/authToken'

const apiBaseUrl = import.meta.env.VITE_API_BASE_URL || ''

export interface NodeItem {
  id: number
  name: string
  protocol: string
  raw_link: string
  server: string
  port: number
  enabled: boolean
  group_id: number | null
  source_type: string
  source_ref: string | null
  upstream_missing: boolean
  fingerprint: string
  settings: Record<string, unknown>
  remark: string
  last_latency_ms: number | null
  last_latency_status: 'ok' | 'timeout' | 'error' | null
  last_latency_message: string | null
  last_latency_tested_at: string | null
  created_at: string
  updated_at: string
}

interface NodeListResponse {
  code: string
  data: NodeItem[]
  pagination?: PaginationMeta
}

export interface NodeListParams {
  page?: number
  page_size?: number
  group_id?: number
  ungrouped?: boolean
  enabled?: boolean
}

interface NodeResponse {
  code: string
  data: NodeItem
}

export interface NodeLatencyResult {
  id: number
  status: 'ok' | 'timeout' | 'error'
  latency_ms: number | null
  message: string | null
  tested_at: string
}

interface NodeLatencyResponse {
  code: string
  data: NodeLatencyResult
}

interface NodeLatencyBatchResponse {
  code: string
  data: NodeLatencyResult[]
}

export interface NodeImportFailure {
  source: string
  reason: string
}

export interface NodeFidelityWarning {
  target: string
  name: string
  protocol: string
  missing_fields: string[]
  changed_fields: string[]
}

export interface NodeImportResponse {
  code: string
  imported: number
  updated: number
  disabled: number
  skipped: number
  failed: number
  template_id: number | null
  template_name: string | null
  fidelity_warnings: NodeFidelityWarning[]
  data: NodeItem[]
  failures: NodeImportFailure[]
}

export interface CreateNodePayload {
  name?: string
  raw_link: string
  group_id?: number | null
  remark?: string
}

export interface ImportNodesFromSubscriptionPayload {
  url: string
  group_id?: number | null
  remark?: string
}

export interface UpdateNodePayload extends CreateNodePayload {
  enabled?: boolean
}

export interface MoveNodesPayload {
  ids: number[]
  group_id?: number | null
}

export async function listNodes(params: NodeListParams = {}) {
  const { data } = await apiClient.get<NodeListResponse>('/api/v1/nodes', { params })
  return data
}

export async function createNode(payload: CreateNodePayload) {
  const { data } = await apiClient.post<NodeResponse>('/api/v1/nodes', payload)
  return data
}

export async function importNodesFromSubscription(payload: ImportNodesFromSubscriptionPayload) {
  const { data } = await apiClient.post<NodeImportResponse>('/api/v1/nodes/import-subscription', payload)
  return data
}

export async function updateNode(id: number, payload: UpdateNodePayload) {
  const { data } = await apiClient.put<NodeResponse>(`/api/v1/nodes/${id}`, payload)
  return data
}

export async function moveNodes(payload: MoveNodesPayload) {
  const { data } = await apiClient.post<NodeListResponse>('/api/v1/nodes/move', payload)
  return data
}

export async function deleteNode(id: number) {
  const { data } = await apiClient.delete<{ code: string; message: string }>(`/api/v1/nodes/${id}`)
  return data
}

export async function testNodeLatency(id: number) {
  const { data } = await apiClient.post<NodeLatencyResponse>(`/api/v1/nodes/${id}/test-latency`)
  return data
}

export async function testNodeLatencyBatch(ids: number[]) {
  const { data } = await apiClient.post<NodeLatencyBatchResponse>('/api/v1/nodes/test-latency', { ids })
  return data
}

export async function testNodeLatencyBatchStream(ids: number[], onResult: (result: NodeLatencyResult) => void) {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    'Cache-Control': 'no-cache',
    Pragma: 'no-cache',
  }
  const token = readAuthToken()
  const csrfToken = readCsrfToken()
  if (token) {
    headers.Authorization = `Bearer ${token}`
  }
  if (csrfToken) {
    headers['X-CSRF-Token'] = csrfToken
  }

  const response = await fetch(`${apiBaseUrl}/api/v1/nodes/test-latency-stream`, {
    method: 'POST',
    credentials: 'include',
    headers,
    body: JSON.stringify({ ids }),
  })

  if (!response.ok) {
    throw await fetchApiError(response)
  }
  if (!response.body) {
    throw new Error('Streaming response is not available.')
  }

  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ''
  while (true) {
    const { done, value } = await reader.read()
    if (done) {
      break
    }

    buffer += decoder.decode(value, { stream: true })
    const lines = buffer.split('\n')
    buffer = lines.pop() ?? ''
    for (const line of lines) {
      const trimmed = line.trim()
      if (trimmed) {
        onResult(JSON.parse(trimmed) as NodeLatencyResult)
      }
    }
  }

  buffer += decoder.decode()
  const trimmed = buffer.trim()
  if (trimmed) {
    onResult(JSON.parse(trimmed) as NodeLatencyResult)
  }
}

export async function cancelAutoLatency() {
  const { data } = await apiClient.post<{ code: string; cancelled: boolean }>('/api/v1/nodes/auto-latency/cancel')
  return data
}

async function fetchApiError(response: Response) {
  let payload: { code?: string; message?: string; error?: string } = {}
  try {
    payload = await response.json()
  } catch {
    payload = {}
  }
  const error = new Error(payload.message ?? payload.error ?? response.statusText) as Error & { code?: string }
  error.code = payload.code
  return error
}
