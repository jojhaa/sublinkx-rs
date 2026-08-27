import apiClient, { type PaginationMeta } from './client'
import type { NodeImportResponse } from './nodes'

export interface UpstreamSubscriptionItem {
  id: number
  name: string
  url: string
  group_id: number | null
  enabled: boolean
  sync_enabled: boolean
  sync_interval_minutes: number
  remark: string
  last_imported_at: string | null
  last_import_status: 'ok' | 'partial' | 'error' | null
  last_import_message: string | null
  last_import_imported: number
  last_import_updated: number
  last_import_disabled: number
  last_import_skipped: number
  last_import_failed: number
  template_id: number | null
  template_name: string | null
  created_at: string
  updated_at: string
}

interface UpstreamSubscriptionListResponse {
  code: string
  data: UpstreamSubscriptionItem[]
  pagination: PaginationMeta
}

interface UpstreamSubscriptionResponse {
  code: string
  data: UpstreamSubscriptionItem
}

interface UpstreamSubscriptionImportResponse {
  code: string
  data: UpstreamSubscriptionItem
  import: NodeImportResponse
}

export interface UpstreamSubscriptionPayload {
  name: string
  url: string
  enabled?: boolean
  sync_enabled?: boolean
  sync_interval_minutes?: number
  remark?: string
}

export async function listUpstreamSubscriptions(params: { page?: number; page_size?: number } = {}) {
  const { data } = await apiClient.get<UpstreamSubscriptionListResponse>('/api/v1/upstream-subscriptions', { params })
  return data
}

export async function createUpstreamSubscription(payload: UpstreamSubscriptionPayload) {
  const { data } = await apiClient.post<UpstreamSubscriptionResponse>('/api/v1/upstream-subscriptions', payload)
  return data
}

export async function updateUpstreamSubscription(id: number, payload: UpstreamSubscriptionPayload) {
  const { data } = await apiClient.put<UpstreamSubscriptionResponse>(`/api/v1/upstream-subscriptions/${id}`, payload)
  return data
}

export async function deleteUpstreamSubscription(id: number, deleteNodes = false) {
  const { data } = await apiClient.delete<{
    code: string
    message: string
    deleted_nodes: number
    detached_nodes: number
  }>(`/api/v1/upstream-subscriptions/${id}`, {
    params: { delete_nodes: deleteNodes },
  })
  return data
}

export async function importUpstreamSubscription(id: number) {
  const { data } = await apiClient.post<UpstreamSubscriptionImportResponse>(`/api/v1/upstream-subscriptions/${id}/import`)
  return data
}
