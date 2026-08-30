import apiClient from './client'
import { readAuthToken, readCsrfToken } from '../utils/authToken'

const apiBaseUrl = import.meta.env.VITE_API_BASE_URL || ''

export interface NodeIpProbeItem {
  node_id: number
  status: 'ok' | 'error'
  ip: string | null
  ip_version: 4 | 6 | null
  country_code: string | null
  country_name: string | null
  country_source: string | null
  intelligence_status: 'syncing' | 'pending' | 'enriched' | 'error' | 'disabled' | null
  intelligence_message: string | null
  message: string | null
  probed_at: string
  country_updated_at: string | null
  intelligence_updated_at: string | null
  updated_at: string
}

interface NodeIpProbeListResponse {
  code: string
  data: NodeIpProbeItem[]
}

export type NodeIpProbeEvent =
  | {
      type: 'job_started'
      total_nodes: number
      provider: string
    }
  | {
      type: 'node_completed'
      id: number
      status: 'ok' | 'error'
      ip: string | null
      ip_version: 4 | 6 | null
      country_code: string | null
      country_name: string | null
      message: string | null
      probed_at: string
    }
  | {
      type: 'intelligence_updated'
      id: number
      country_code: string | null
      country_name: string | null
      country_source: string | null
      intelligence_status: NodeIpProbeItem['intelligence_status']
      intelligence_message: string | null
      intelligence_updated_at: string | null
    }
  | {
      type: 'job_completed'
      total_nodes: number
      succeeded: number
      failed: number
      cancelled: boolean
    }

export async function listNodeIpProbes() {
  const { data } = await apiClient.get<NodeIpProbeListResponse>('/api/v1/node-ip-probes')
  return data
}

export async function streamNodeIpProbes(
  ids: number[],
  onEvent: (event: NodeIpProbeEvent) => void,
  signal?: AbortSignal,
) {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    'Cache-Control': 'no-cache',
    Pragma: 'no-cache',
  }
  const token = readAuthToken()
  const csrfToken = readCsrfToken()
  if (token) headers.Authorization = `Bearer ${token}`
  if (csrfToken) headers['X-CSRF-Token'] = csrfToken

  const response = await fetch(`${apiBaseUrl}/api/v1/node-ip-probes/stream`, {
    method: 'POST',
    credentials: 'include',
    headers,
    body: JSON.stringify({ ids }),
    signal,
  })
  if (!response.ok) throw await fetchApiError(response)
  if (!response.body) throw new Error('Streaming response is not available.')

  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ''
  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    buffer += decoder.decode(value, { stream: true })
    const lines = buffer.split('\n')
    buffer = lines.pop() ?? ''
    for (const line of lines) {
      const trimmed = line.trim()
      if (trimmed) onEvent(JSON.parse(trimmed) as NodeIpProbeEvent)
    }
  }
  buffer += decoder.decode()
  if (buffer.trim()) onEvent(JSON.parse(buffer.trim()) as NodeIpProbeEvent)
}

export async function cancelNodeIpProbes() {
  const { data } = await apiClient.post<{ code: string; cancelled: boolean }>('/api/v1/node-ip-probes/cancel')
  return data
}

export interface NodeIpIntelligenceStatus {
  code: string
  enabled: boolean
  base_url: string | null
  source_key: string | null
}

export async function getNodeIpIntelligenceStatus() {
  const { data } = await apiClient.get<NodeIpIntelligenceStatus>('/api/v1/node-ip-probes/intelligence/status')
  return data
}

export async function refreshNodeIpIntelligence(ids: number[]) {
  const { data } = await apiClient.post<{
    code: string
    updated: number
    pending: number
    failed: number
    data: NodeIpProbeItem[]
  }>('/api/v1/node-ip-probes/intelligence/refresh', { ids })
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
