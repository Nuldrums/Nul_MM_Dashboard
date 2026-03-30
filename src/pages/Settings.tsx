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
import { apiFetch } from '../hooks/useApi';
import { PLATFORM_NAMES, PLATFORM_COLORS } from '../lib/constants';
import ThemeSwitcher from '../components/ThemeSwitcher';

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

  // AI Config
  const [claudeKey, setClaudeKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [aiModel, setAiModel] = useState('claude-sonnet-4-20250514');
  const [autoAnalysis, setAutoAnalysis] = useState(true);

  // Platform credentials (simplified for UI)
  const [editingPlatform, setEditingPlatform] = useState<string | null>(null);
  const [credentialValue, setCredentialValue] = useState('');

  const [confirmReset, setConfirmReset] = useState(false);

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
    try {
      await apiFetch(`/settings/platforms/${platform}`, {
        method: 'PUT',
        body: JSON.stringify({ credential: credentialValue }),
      });
      setEditingPlatform(null);
      setCredentialValue('');
      refetchHealth();
    } catch {
      // handle
    }
  };

  const handleSaveAiConfig = async () => {
    try {
      await apiFetch('/settings/ai', {
        method: 'PUT',
        body: JSON.stringify({
          api_key: claudeKey || undefined,
          model: aiModel,
          auto_analysis: autoAnalysis,
        }),
      });
    } catch {
      // handle
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
    'x',
    'youtube',
    'discord',
    'tiktok',
    'instagram',
    'linkedin',
  ];

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

          return (
            <div key={platform} className="platform-status-row">
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
                      <CheckCircle
                        size={14}
                        style={{ color: 'var(--success)' }}
                      />
                      <span
                        style={{
                          fontSize: '0.8rem',
                          color: 'var(--success)',
                        }}
                      >
                        Connected
                      </span>
                    </>
                  ) : (
                    <>
                      <XCircle
                        size={14}
                        style={{ color: 'var(--text-tertiary)' }}
                      />
                      <span
                        style={{
                          fontSize: '0.8rem',
                          color: 'var(--text-tertiary)',
                        }}
                      >
                        Not configured
                      </span>
                    </>
                  )}
                </span>
              </div>
              <div className="flex-gap">
                {isEditing ? (
                  <div className="flex-gap" style={{ gap: 6 }}>
                    <input
                      className="form-input"
                      type="text"
                      value={credentialValue}
                      onChange={(e) =>
                        setCredentialValue(e.target.value)
                      }
                      placeholder="API key or token"
                      style={{ width: 200, padding: '4px 8px', fontSize: '0.8rem' }}
                    />
                    <button
                      className="btn btn-primary btn-sm"
                      onClick={() =>
                        handleSaveCredential(platform)
                      }
                    >
                      Save
                    </button>
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={() => {
                        setEditingPlatform(null);
                        setCredentialValue('');
                      }}
                    >
                      Cancel
                    </button>
                  </div>
                ) : (
                  <div className="flex-gap" style={{ gap: 6 }}>
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={() => handleTestPlatform(platform)}
                    >
                      <RefreshCw size={12} /> Test
                    </button>
                    <button
                      className="btn btn-secondary btn-sm"
                      onClick={() => setEditingPlatform(platform)}
                    >
                      Edit
                    </button>
                  </div>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {/* AI Configuration */}
      <div className="settings-section">
        <h3>
          <Brain size={18} /> AI Configuration
        </h3>
        <div className="form-group">
          <label>Claude API Key</label>
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
        <button
          className="btn btn-primary btn-sm"
          onClick={handleSaveAiConfig}
        >
          Save AI Settings
        </button>
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
