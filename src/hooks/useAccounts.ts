import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from './useApi';
import type { ProfileAccount } from '../lib/types';

export function useAccounts(profileId: string | null | undefined) {
  return useQuery<ProfileAccount[]>({
    queryKey: ['accounts', profileId ?? 'none'],
    queryFn: () => apiFetch<ProfileAccount[]>(`/profiles/${profileId}/accounts`),
    enabled: !!profileId,
  });
}

export function useCreateAccount(profileId: string | null | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: { platform: string; account_handle: string }) =>
      apiFetch<ProfileAccount>(`/profiles/${profileId}/accounts`, {
        method: 'POST',
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['accounts', profileId] });
    },
  });
}

export function useDeleteAccount(profileId: string | null | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (accountId: string) =>
      apiFetch<void>(`/profile-accounts/${accountId}`, { method: 'DELETE' }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['accounts', profileId] });
    },
  });
}

export async function startTikTokOAuth(profileId: string): Promise<{ auth_url: string; state: string }> {
  return apiFetch<{ auth_url: string; state: string }>(
    `/oauth/tiktok/start?profile_id=${profileId}`
  );
}
