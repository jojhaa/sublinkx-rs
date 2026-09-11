import apiClient, { type PaginationMeta } from './client'
import type { NodeItem } from './nodes'

export interface SubscriptionItem {
  id: number
  name: string
  token: string
  description: string
  default_client: string | null
  include_rules: boolean
  template_id: number | null
  group_id: number | null
  enabled: boolean
  expires_at: string | null
  status: 'active' | 'disabled' | 'expired'
  portal_enabled: boolean
  portal_slug: string | null
  portal_access_code_set: boolean
  node_group_ids: number[]
  node_ids: number[]
  nodes?: NodeItem[]
  created_at: string
  updated_at: string
}

interface SubscriptionListResponse {
  code: string
  data: SubscriptionItem[]
  pagination: PaginationMeta
}

export interface SubscriptionListParams {
  page?: number
  page_size?: number
  group_id?: number
  ungrouped?: boolean
}

interface SubscriptionResponse {
  code: string
  data: SubscriptionItem
}

export interface SubscriptionPayload {
  name: string
  description?: string
  default_client?: string | null
  include_rules?: boolean
  template_id?: number | null
  group_id?: number | null
  enabled?: boolean
  expires_at?: string | null
  portal_enabled?: boolean
  portal_access_code?: string
  node_group_ids?: number[]
  node_ids: number[]
}

export interface SubscriptionPortalLink {
  target: string
  label: string
  path: string
  full_profile: boolean
}

export interface SubscriptionPortalData {
  include_rules: boolean
  name: string
  description: string
  expires_at: string | null
  default_client: string | null
  links: SubscriptionPortalLink[]
}

export async function unlockSubscriptionPortal(portalSlug: string, accessCode: string) {
  const { data } = await apiClient.post<{ code: string; data: SubscriptionPortalData }>(
    `/api/public/v1/subscription-portals/${encodeURIComponent(portalSlug)}/unlock`,
    { access_code: accessCode },
  )
  return data
}

export async function listSubscriptions(params: SubscriptionListParams = {}) {
  const { data } = await apiClient.get<SubscriptionListResponse>('/api/v1/subscriptions', { params })
  return data
}

export async function createSubscription(payload: SubscriptionPayload) {
  const { data } = await apiClient.post<SubscriptionResponse>('/api/v1/subscriptions', payload)
  return data
}

export async function updateSubscription(id: number, payload: SubscriptionPayload) {
  const { data } = await apiClient.put<SubscriptionResponse>(`/api/v1/subscriptions/${id}`, payload)
  return data
}

export async function deleteSubscription(id: number) {
  const { data } = await apiClient.delete<{ code: string; message: string }>(
    `/api/v1/subscriptions/${id}`,
  )
  return data
}

export async function rotateSubscriptionToken(id: number) {
  const { data } = await apiClient.post<SubscriptionResponse>(
    `/api/v1/subscriptions/${id}/rotate-token`,
  )
  return data
}

export async function renewSubscription(id: number, days: number) {
  const { data } = await apiClient.post<SubscriptionResponse>(
    `/api/v1/subscriptions/${id}/renew`,
    { days },
  )
  return data
}

export async function fetchSubscriptionExport(id: number, target: string | null, mode: string) {
  const { data, headers } = await apiClient.get<Blob>(`/api/v1/subscriptions/${id}/export`, {
    params: {
      ...(target ? { target } : {}),
      mode,
    },
    responseType: 'blob',
  })
  return {
    blob: data,
    contentType: headers['content-type'] as string | undefined,
  }
}
