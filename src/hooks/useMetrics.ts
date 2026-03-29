import { useQuery } from '@tanstack/react-query';
import { apiFetch } from './useApi';
import type { MetricSnapshot, MetricsSummary } from '../lib/types';

export function usePostMetrics(postId: string) {
  return useQuery<MetricSnapshot[]>({
    queryKey: ['metrics', 'post', postId],
    queryFn: () => apiFetch<MetricSnapshot[]>(`/posts/${postId}/metrics`),
    enabled: !!postId,
  });
}

export function useCampaignMetrics(campaignId: string) {
  return useQuery<MetricsSummary>({
    queryKey: ['metrics', 'campaign', campaignId],
    queryFn: () => apiFetch<MetricsSummary>(`/campaigns/${campaignId}/metrics`),
    enabled: !!campaignId,
  });
}
