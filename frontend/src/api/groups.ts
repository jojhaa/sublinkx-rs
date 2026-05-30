import apiClient from './client'

export interface GroupItem {
  id: number
  name: string
  sort_order: number
  created_at: string
  updated_at: string
}

interface GroupListResponse {
  code: string
  data: GroupItem[]
}

interface GroupResponse {
  code: string
  data: GroupItem
}

export interface GroupPayload {
  name: string
  sort_order?: number
}

export async function listNodeGroups() {
  const { data } = await apiClient.get<GroupListResponse>('/api/v1/node-groups')
  return data
}

export async function createNodeGroup(payload: GroupPayload) {
  const { data } = await apiClient.post<GroupResponse>('/api/v1/node-groups', payload)
  return data
}

export async function updateNodeGroup(id: number, payload: GroupPayload) {
  const { data } = await apiClient.put<GroupResponse>(`/api/v1/node-groups/${id}`, payload)
  return data
}

export async function deleteNodeGroup(id: number) {
  const { data } = await apiClient.delete<{ code: string; message: string }>(`/api/v1/node-groups/${id}`)
  return data
}

export async function listSubscriptionGroups() {
  const { data } = await apiClient.get<GroupListResponse>('/api/v1/subscription-groups')
  return data
}

export async function createSubscriptionGroup(payload: GroupPayload) {
  const { data } = await apiClient.post<GroupResponse>('/api/v1/subscription-groups', payload)
  return data
}

export async function updateSubscriptionGroup(id: number, payload: GroupPayload) {
  const { data } = await apiClient.put<GroupResponse>(`/api/v1/subscription-groups/${id}`, payload)
  return data
}

export async function deleteSubscriptionGroup(id: number) {
  const { data } = await apiClient.delete<{ code: string; message: string }>(
    `/api/v1/subscription-groups/${id}`,
  )
  return data
}
