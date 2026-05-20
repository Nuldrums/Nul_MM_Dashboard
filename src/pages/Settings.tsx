import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  Link2,
  Brain,
  Palette,
  Database,
  CheckCircle,
  XCircle,
  Eye,
  EyeOff,
  Download,
  Trash2,
  RefreshCw,
} from 'lucide-react';
import { open as openExternal } from '@tauri-apps/plugin-shell';
import { apiFetch } from '../hooks/useApi';
import { useAccounts, useCreateAccount, useDeleteAccount, startTikTokOAuth } from '../hooks/useAccounts';
import { useActiveProfile } from '../hooks/useActiveProfile';
import { PLATFORM_NAMES, PLATFORM_COLORS } from '../lib/constants';
import ThemeSwitcher from '../components/ThemeSwitcher';

const FEED_CAPABLE_PLATFORMS = [
  { value: 'youtube', label: 'YouTube', supportsManualHandle: true },
  { value: 'x', label: 'X / Twitter', supportsManualHandle: true },
  { value: 'tiktok', label: 'TikTok', supportsManualHandle: false },
];

interface HealthStatus {
  platforms: {
    platform: string;
    status: 'connected' | 'not_configured' | 'error';
    message?: string;
  }[];
  ai: {
    status: 'connected' | 'not_configured' | 'error';
    model?: string;
  };
  database: {
    path: string;
    size_mb: number;
  };
}

export default function SettingsPage() {
  const { data: health, refetch: refetchHealth } = useQuery<HealthStatus>({
    queryKey: ['settings', 'health'],
    queryFn: () => apiFetch<HealthStatus>('/settings/health'),
  });

  const { activeProfileId, activeProfile } = useActiveProfile();
  const { data: accounts = [] } = useAccounts(activeProfileId);
  const createAccount = useCreateAccount(activeProfileId);
  const deleteAccount = useDeleteAccount(activeProfileId);

  const [newAccountPlatform, setNewAccountPlatform] = useState<string>('youtube');
  const [newAccountHandle, setNewAccountHandle] = useState('');

  const handleAddAccount = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newAccountHandle.trim() || !activeProfileId) return;
    try {
      await createAccount.mutateAsync({
        platform: newAccountPlatform,
        account_handle: newAccountHandle.trim(),
      });
      setNewAccountHandle('');
    } catch {
      // Surfaced via createAccount.error below
    }
  };

  const handleConnectTikTok = async () => {
    if (!activeProfileId) return;
    try {
      const { auth_url } = await startTikTokOAuth(activeProfileId);
      // Tauri's webview blocks window.open() to external URLs — use the shell plugin
      // to hand the URL to the system default browser instead.
      await openExternal(auth_url);
    } catch (e) {
      alert((e as Error).message);
    }
  };

  // AI Config
  const [aiProvider, setAiProvider] = useState<'cli' | 'api'>('cli');
  const [claudeKey, setClaudeKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [aiModel, setAiModel] = useState('claude-sonnet-4-20250514');
  const [autoAnalysis, setAutoAnalysis] = useState(true);

  // Platform credentials — schema per platform
  const [editingPlatform, setEditingPlatform] = useState<string | null>(null);
  const [credentialFields, setCredentialFields] = useState<Record<string, string>>({});

  const [confirmReset, setConfirmReset] = useState(false);
  const [aiTestLoading, setAiTestLoading] = useState(false);
  const [aiTestResult, setAiTestResult] = useState<{
    success: boolean;
    provider?: string;
    model?: string;
    response?: string;
    error?: string;
  } | null>(null);

  const handleTestPlatform = async (platform: string) => {
    try {
      await apiFetch(`/settings/platforms/${platform}/test`, {
        method: 'POST',
      });
      refetchHealth();
    } catch {
      // handle
    }
  };

  const handleSaveCredential = async (platform: string) => {
    const credentials = Object.fromEntries(
      Object.entries(credentialFields).filter(([, v]) => v.trim() !== '')
    );
    if (Object.keys(credentials).length === 0) {
      alert('Fill in at least one field before saving.');
      return;
    }
    try {
      await apiFetch(`/settings/platform/${platform}`, {
        method: 'PUT',
        body: JSON.stringify({ credentials, is_enabled: true }),
      });
      setEditingPlatform(null);
      setCredentialFields({});
      refetchHealth();
    } catch (err) {
      alert(`Failed to save ${platform} credentials: ${err instanceof Error ? err.message : err}`);
    }
  };

  const handleSaveAiConfig = async () => {
    try {
      await apiFetch('/settings/ai', {
        method: 'PUT',
        body: JSON.stringify({
          provider: aiProvider,
          api_key: claudeKey || undefined,
          model: aiModel,
          auto_analysis: autoAnalysis,
        }),
      });
    } catch {
      // handle
    }
  };

  const handleTestAi = async () => {
    setAiTestLoading(true);
    setAiTestResult(null);
    try {
      const result = await apiFetch<{
        success: boolean;
        provider?: string;
        model?: string;
        response?: string;
        error?: string;
      }>('/ai/test', { method: 'POST' });
      setAiTestResult(result ?? { success: false, error: 'No response from server' });
    } catch (e) {
      setAiTestResult({ success: false, error: String(e) });
    } finally {
      setAiTestLoading(false);
    }
  };

  const handleExportData = async () => {
    try {
      const blob = await fetch(
        'http://localhost:31415/api/settings/export'
      ).then((r) => r.blob());
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'meem-export.json';
      a.click();
      URL.revokeObjectURL(url);
    } catch {
      // handle
    }
  };

  const handleResetKB = async () => {
    if (!confirmReset) {
      setConfirmReset(true);
      return;
    }
    try {
      await apiFetch('/ai/knowledge-base/reset', { method: 'POST' });
      setConfirmReset(false);
    } catch {
      // handle
    }
  };

  const platforms = [
    'reddit',
    'twitter',
    'youtube',
    'discord',
    'tiktok',
    'instagram',
    'linkedin',
  ];

  // Per-platform credential field schema. Each entry is { key, label, type, placeholder }.
  // Empty array = no API support (manual-only platform).
  const CREDENTIAL_FIELDS: Record<string, { key: string; label: string; type: 'text' | 'password'; placeholder: string }[]> = {
    reddit: [
      { key: 'client_id', label: 'Client ID', type: 'text', placeholder: 'reddit app client ID' },
      { key: 'client_secret', label: 'Client Secret', type: 'password', placeholder: 'reddit app client secret' },
      { key: 'username', label: 'Username', type: 'text', placeholder: 'reddit username' },
      { key: 'password', label: 'Password', type: 'password', placeholder: 'reddit password' },
    ],
    twitter: [
      { key: 'bearer_token', label: 'Bearer Token', type: 'password', placeholder: 'AAAAAAAA...' },
    ],
    youtube: [
      { key: 'api_key', label: 'API Key', type: 'password', placeholder: 'AIza...' },
    ],
    tiktok: [
      { key: 'client_key', label: 'Client Key', type: 'text', placeholder: 'TikTok app client_key' },
      { key: 'client_secret', label: 'Client Secret', type: 'password', placeholder: 'TikTok app client_secret' },
    ],
  };

  return (
    <div>
      <div className="page-header">
        <h2>Settings</h2>
        <p>Configure your MEEM Marketing engine</p>
      </div>

      {/* Platform Connections */}
      <div className="settings-section">
        <h3>
          <Link2 size={18} /> Platform Connections
        </h3>
        {platforms.map((platform) => {
          const pHealth = health?.platforms?.find(
            (p) => p.platform === platform
          );
          const isConnected = pHealth?.status === 'connected';
          const isEditing = editingPlatform === platform;

          const schema = CREDENTIAL_FIELDS[platform];
          const isApiCapable = !!schema && schema.length > 0;

          return (
            <div key={platform} style={{ borderBottom: '1px solid var(--border-light)', padding: '8px 0' }}>
              <div className="platform-status-row" style={{ borderBottom: 'none' }}>
                <div className="flex-gap" style={{ gap: 12 }}>
                  <span
                    style={{
                      width: 10,
                      height: 10,
                      borderRadius: '50%',
                      backgroundColor:
                        PLATFORM_COLORS[platform] ??
                        PLATFORM_COLORS.other,
                      flexShrink: 0,
                    }}
                  />
                  <span style={{ fontWeight: 500, fontSize: '0.875rem', minWidth: 100 }}>
                    {PLATFORM_NAMES[platform] ?? platform}
                  </span>
                  <span className="flex-gap" style={{ gap: 4 }}>
                    {isConnected ? (
                      <>
                        <CheckCircle size={14} style={{ color: 'var(--success)' }} />
                        <span style={{ fontSize: '0.8rem', color: 'var(--success)' }}>Connected</span>
                      </>
                    ) : (
                      <>
                        <XCircle size={14} style={{ color: 'var(--text-tertiary)' }} />
                        <span style={{ fontSize: '0.8rem', color: 'var(--text-tertiary)' }}>
                          {isApiCapable ? 'Not configured' : 'Manual entry only'}
                        </span>
                      </>
                    )}
                  </span>
                </div>
                <div className="flex-gap">
                  {isApiCapable && !isEditing && (
                    <div className="flex-gap" style={{ gap: 6 }}>
                      <button
                        className="btn btn-ghost btn-sm"
                        onClick={() => handleTestPlatform(platform)}
                      >
                        <RefreshCw size={12} /> Test
                      </button>
                      <button
                        className="btn btn-secondary btn-sm"
                        onClick={() => {
                          setEditingPlatform(platform);
                          setCredentialFields(
                            Object.fromEntries(schema.map((f) => [f.key, '']))
                          );
                        }}
                      >
                        Edit
                      </button>
                    </div>
                  )}
                </div>
              </div>

              {isEditing && schema && (
                <div style={{ padding: '12px 0 8px 22px', display: 'flex', flexDirection: 'column', gap: 8 }}>
                  {schema.map((field) => (
                    <div key={field.key} className="flex-gap" style={{ gap: 8, alignItems: 'center' }}>
                      <label style={{ fontSize: '0.8rem', minWidth: 110, color: 'var(--text-secondary)' }}>
                        {field.label}
                      </label>
                      <input
                        className="form-input"
                        type={field.type}
                        value={credentialFields[field.key] ?? ''}
                        onChange={(e) =>
                          setCredentialFields((prev) => ({ ...prev, [field.key]: e.target.value }))
                        }
                        placeholder={field.placeholder}
                        style={{ flex: 1, maxWidth: 360, padding: '4px 8px', fontSize: '0.8rem' }}
                      />
                    </div>
                  ))}
                  <div className="flex-gap" style={{ gap: 6, marginTop: 4, paddingLeft: 118 }}>
                    <button
                      className="btn btn-primary btn-sm"
                      onClick={() => handleSaveCredential(platform)}
                    >
                      Save
                    </button>
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={() => {
                        setEditingPlatform(null);
                        setCredentialFields({});
                      }}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* Connected Accounts (per profile) */}
      <div className="settings-section">
        <h3>
          <Link2 size={18} /> Connected Accounts
          {activeProfile && (
            <span className="text-muted" style={{ fontWeight: 400, fontSize: '0.8rem', marginLeft: 8 }}>
              · {activeProfile.name}
            </span>
          )}
        </h3>
        <p className="text-muted" style={{ fontSize: '0.8rem', marginBottom: 12 }}>
          Social accounts owned by the active profile. Feeds reference these to auto-discover new posts.
        </p>

        {!activeProfileId && (
          <p className="text-muted" style={{ fontSize: '0.85rem' }}>Select a profile to manage its connected accounts.</p>
        )}

        {activeProfileId && (
          <>
            {accounts.length > 0 && (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6, marginBottom: 12 }}>
                {accounts.map((acc) => (
                  <div
                    key={acc.id}
                    className="flex-between"
                    style={{
                      padding: '8px 10px',
                      background: 'var(--bg-tertiary)',
                      borderRadius: 'var(--radius-sm)',
                      fontSize: '0.85rem',
                    }}
                  >
                    <div className="flex-gap" style={{ gap: 10, alignItems: 'center' }}>
                      <span
                        style={{
                          width: 8, height: 8, borderRadius: '50%',
                          backgroundColor: PLATFORM_COLORS[acc.platform] ?? PLATFORM_COLORS.other,
                        }}
                      />
                      <span style={{ fontWeight: 500, minWidth: 90 }}>
                        {PLATFORM_NAMES[acc.platform] ?? acc.platform}
                      </span>
                      <span>{acc.account_handle}</span>
                      {acc.has_oauth ? (
                        <span style={{ color: 'var(--success)', fontSize: '0.75rem' }}>
                          · OAuth authorized
                        </span>
                      ) : null}
                    </div>
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={() => {
                        if (confirm(`Remove ${acc.platform} account ${acc.account_handle}? Any feeds using it will be deleted too.`)) {
                          deleteAccount.mutate(acc.id);
                        }
                      }}
                      title="Remove account"
                    >
                      <Trash2 size={14} />
                    </button>
                  </div>
                ))}
              </div>
            )}

            <form onSubmit={handleAddAccount} style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'flex-end' }}>
              <div className="form-group" style={{ margin: 0 }}>
                <label style={{ fontSize: '0.75rem' }}>Platform</label>
                <select
                  className="form-select"
                  value={newAccountPlatform}
                  onChange={(e) => setNewAccountPlatform(e.target.value)}
                  style={{ minWidth: 140 }}
                >
                  {FEED_CAPABLE_PLATFORMS.map((p) => (
                    <option key={p.value} value={p.value}>{p.label}</option>
                  ))}
                </select>
              </div>
              {FEED_CAPABLE_PLATFORMS.find(p => p.value === newAccountPlatform)?.supportsManualHandle ? (
                <>
                  <div className="form-group" style={{ margin: 0, flex: 1, minWidth: 200 }}>
                    <label style={{ fontSize: '0.75rem' }}>Handle or channel ID</label>
                    <input
                      className="form-input"
                      value={newAccountHandle}
                      onChange={(e) => setNewAccountHandle(e.target.value)}
                      placeholder={newAccountPlatform === 'youtube' ? '@channel or UCxxxxxxxx' : '@username'}
                    />
                  </div>
                  <button
                    type="submit"
                    className="btn btn-primary btn-sm"
                    disabled={createAccount.isPending || !newAccountHandle.trim()}
                  >
                    {createAccount.isPending ? 'Verifying...' : 'Add'}
                  </button>
                </>
              ) : (
                <button
                  type="button"
                  className="btn btn-primary btn-sm"
                  onClick={handleConnectTikTok}
                >
                  Connect TikTok via OAuth
                </button>
              )}
            </form>
            {createAccount.isError && (
              <p style={{ color: 'var(--danger)', fontSize: '0.825rem', marginTop: 8 }}>
                {(createAccount.error as Error).message}
              </p>
            )}
          </>
        )}
      </div>

      {/* AI Configuration */}
      <div className="settings-section">
        <h3>
          <Brain size={18} /> AI Configuration
        </h3>

        <div className="form-group">
          <label>AI Provider</label>
          <select
            className="form-select"
            value={aiProvider}
            onChange={(e) => setAiProvider(e.target.value as 'cli' | 'api')}
          >
            <option value="cli">Claude CLI (Subscription) — API key as fallback</option>
            <option value="api">API Key Only</option>
          </select>
          <p className="text-muted" style={{ fontSize: '0.75rem', marginTop: 4 }}>
            {aiProvider === 'cli'
              ? 'Uses your Claude subscription via CLI. Falls back to API key if rate limited.'
              : 'Uses Anthropic API directly. Requires an API key.'}
          </p>
        </div>

        <div className="form-group">
          <label>
            Anthropic API Key
            {aiProvider === 'cli' && (
              <span className="text-muted" style={{ fontWeight: 400, fontSize: '0.8rem' }}>
                {' '}(optional — fallback for rate limits)
              </span>
            )}
          </label>
          <div className="flex-gap" style={{ gap: 6 }}>
            <input
              className="form-input"
              type={showKey ? 'text' : 'password'}
              value={claudeKey}
              onChange={(e) => setClaudeKey(e.target.value)}
              placeholder={
                health?.ai?.status === 'connected'
                  ? 'Key configured (hidden)'
                  : 'sk-ant-...'
              }
              style={{ flex: 1 }}
            />
            <button
              className="btn btn-ghost btn-sm"
              onClick={() => setShowKey(!showKey)}
              type="button"
            >
              {showKey ? <EyeOff size={16} /> : <Eye size={16} />}
            </button>
          </div>
        </div>

        <div className="form-group">
          <label>Model</label>
          <select
            className="form-select"
            value={aiModel}
            onChange={(e) => setAiModel(e.target.value)}
          >
            <option value="claude-sonnet-4-20250514">
              Claude Sonnet 4 (Recommended)
            </option>
            <option value="claude-opus-4-20250514">
              Claude Opus 4
            </option>
            <option value="claude-3-5-haiku-20241022">
              Claude 3.5 Haiku (Fast)
            </option>
          </select>
        </div>

        <div className="form-group">
          <label
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 8,
              cursor: 'pointer',
            }}
          >
            <input
              type="checkbox"
              checked={autoAnalysis}
              onChange={(e) => setAutoAnalysis(e.target.checked)}
              style={{ width: 16, height: 16 }}
            />
            Auto-analyze after fetching metrics
          </label>
        </div>

        <div className="flex-gap" style={{ gap: 8 }}>
          <button
            className="btn btn-primary btn-sm"
            onClick={handleSaveAiConfig}
          >
            Save AI Settings
          </button>
          <button
            className="btn btn-secondary btn-sm"
            onClick={handleTestAi}
            disabled={aiTestLoading}
          >
            <RefreshCw size={14} className={aiTestLoading ? 'spin' : ''} />{' '}
            {aiTestLoading ? 'Testing...' : 'Test AI Connection'}
          </button>
        </div>

        {aiTestResult && (
          <div
            style={{
              marginTop: 12,
              padding: '10px 14px',
              borderRadius: 8,
              fontSize: '0.85rem',
              backgroundColor: aiTestResult.success
                ? 'color-mix(in srgb, var(--success) 15%, transparent)'
                : 'color-mix(in srgb, var(--danger, #e53e3e) 15%, transparent)',
              border: `1px solid ${aiTestResult.success ? 'var(--success)' : 'var(--danger, #e53e3e)'}`,
            }}
          >
            {aiTestResult.success ? (
              <>
                <div style={{ fontWeight: 600, marginBottom: 4 }}>
                  <CheckCircle size={14} style={{ color: 'var(--success)', verticalAlign: 'middle', marginRight: 6 }} />
                  Connection successful
                </div>
                <div className="text-muted" style={{ fontSize: '0.8rem' }}>
                  Provider: {aiTestResult.provider === 'cli' ? 'Claude CLI (subscription)' : 'API'} &middot; Model: {aiTestResult.model}
                </div>
                <div style={{ marginTop: 6, fontStyle: 'italic' }}>
                  &ldquo;{aiTestResult.response}&rdquo;
                </div>
              </>
            ) : (
              <>
                <div style={{ fontWeight: 600, marginBottom: 4 }}>
                  <XCircle size={14} style={{ color: 'var(--danger, #e53e3e)', verticalAlign: 'middle', marginRight: 6 }} />
                  Connection failed
                </div>
                <div style={{ fontSize: '0.8rem' }}>{aiTestResult.error}</div>
              </>
            )}
          </div>
        )}
      </div>

      {/* Appearance */}
      <div className="settings-section">
        <h3>
          <Palette size={18} /> Appearance
        </h3>
        <ThemeSwitcher />
      </div>

      {/* Data */}
      <div className="settings-section">
        <h3>
          <Database size={18} /> Data
        </h3>
        {health?.database && (
          <div style={{ marginBottom: 16 }}>
            <p className="text-muted" style={{ marginBottom: 4 }}>
              Database location: {health.database.path}
            </p>
            <p className="text-muted">
              Size: {health.database.size_mb.toFixed(1)} MB
            </p>
          </div>
        )}
        <div className="flex-gap" style={{ gap: 10 }}>
          <button
            className="btn btn-secondary btn-sm"
            onClick={handleExportData}
          >
            <Download size={14} /> Export Data
          </button>
          <button
            className={`btn ${confirmReset ? 'btn-danger' : 'btn-secondary'} btn-sm`}
            onClick={handleResetKB}
          >
            <Trash2 size={14} />{' '}
            {confirmReset
              ? 'Confirm Reset Knowledge Base'
              : 'Reset Knowledge Base'}
          </button>
          {confirmReset && (
            <button
              className="btn btn-ghost btn-sm"
              onClick={() => setConfirmReset(false)}
            >
              Cancel
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
