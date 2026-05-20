import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from './useApi';
import type { CampaignFeed } from '../lib/types';

export function useFeeds(campaignId: string) {
  return useQuery<CampaignFeed[]>({
    queryKey: ['feeds', campaignId],
    queryFn: () => apiFetch<CampaignFeed[]>(`/campaigns/${campaignId}/feeds`),
    enabled: !!campaignId,
  });
}

export function useCreateFeed(campaignId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: { platform: string; account_handle: string; content_type: string }) =>
      apiFetch<CampaignFeed>(`/campaigns/${campaignId}/feeds`, {
        method: 'POST',
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['feeds', campaignId] });
    },
  });
}

export function useDeleteFeed(campaignId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (feedId: string) =>
      apiFetch<void>(`/feeds/${feedId}`, { method: 'DELETE' }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['feeds', campaignId] });
    },
  });
}
