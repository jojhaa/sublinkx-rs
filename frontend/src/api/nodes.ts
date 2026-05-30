import apiClient from './client'

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

interface NodeImportFailure {
  source: string
  reason: string
}

interface NodeFidelityWarning {
  target: string
  name: string
  protocol: string
  missing_fields: string[]
  changed_fields: string[]
}

interface NodeImportResponse {
  code: string
  imported: number
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

export async function listNodes() {
  const { data } = await apiClient.get<NodeListResponse>('/api/v1/nodes')
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
