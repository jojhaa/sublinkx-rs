import apiClient from './client'
import { readAuthToken, readCsrfToken } from '../utils/authToken'

const apiBaseUrl = import.meta.env.VITE_API_BASE_URL || ''

export interface ConnectivityTestPreset {
  id: string
  name: string
  url: string
  is_system_default: boolean
}

interface ConnectivityTestPresetResponse {
  code: string
  data: ConnectivityTestPreset[]
}

export interface ConnectivityTestPayload {
  ids: number[]
  target_id: string
  rounds: 1 | 3 | 5
  sync_last_latency: boolean
}

export type ConnectivityTestEvent =
  | {
      type: 'job_started'
      total_nodes: number
      rounds: number
      target_id: string
      target_url: string
      concurrency: number
    }
  | {
      type: 'sample_completed'
      id: number
      round: number
      status: 'ok' | 'error' | 'cancelled'
      latency_ms: number | null
      message: string | null
    }
  | {
      type: 'node_completed'
      id: number
      status: 'ok' | 'partial' | 'error' | 'cancelled'
      min_ms: number | null
      average_ms: number | null
      max_ms: number | null
      jitter_ms: number | null
      success_rate: number
      succeeded: number
      failed: number
      message: string | null
      tested_at: string
    }
  | {
      type: 'job_completed'
      total_nodes: number
      succeeded: number
      failed: number
      cancelled: boolean
    }

export async function listConnectivityTestPresets() {
  const { data } = await apiClient.get<ConnectivityTestPresetResponse>('/api/v1/connectivity-tests/presets')
  return data
}

export async function streamConnectivityTest(
  payload: ConnectivityTestPayload,
  onEvent: (event: ConnectivityTestEvent) => void,
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

  const response = await fetch(`${apiBaseUrl}/api/v1/connectivity-tests/stream`, {
    method: 'POST',
    credentials: 'include',
    headers,
    body: JSON.stringify(payload),
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
      if (trimmed) onEvent(JSON.parse(trimmed) as ConnectivityTestEvent)
    }
  }
  buffer += decoder.decode()
  const trimmed = buffer.trim()
  if (trimmed) onEvent(JSON.parse(trimmed) as ConnectivityTestEvent)
}

export async function cancelConnectivityTest() {
  const { data } = await apiClient.post<{ code: string; cancelled: boolean }>('/api/v1/connectivity-tests/cancel')
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
