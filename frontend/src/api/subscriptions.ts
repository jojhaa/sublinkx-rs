import apiClient from './client'
import type { NodeItem } from './nodes'

export interface SubscriptionItem {
  id: number
  name: string
  token: string
  description: string
  default_client: string | null
  template_id: number | null
  group_id: number | null
  enabled: boolean
  expires_at: string | null
  status: 'active' | 'disabled' | 'expired'
  node_ids: number[]
  nodes: NodeItem[]
  created_at: string
  updated_at: string
}

interface SubscriptionListResponse {
  code: string
  data: SubscriptionItem[]
}

interface SubscriptionResponse {
  code: string
  data: SubscriptionItem
}

export interface SubscriptionPayload {
  name: string
  description?: string
  default_client?: string | null
  template_id?: number | null
  group_id?: number | null
  enabled?: boolean
  expires_at?: string | null
  node_ids: number[]
}

export async function listSubscriptions() {
  const { data } = await apiClient.get<SubscriptionListResponse>('/api/v1/subscriptions')
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
