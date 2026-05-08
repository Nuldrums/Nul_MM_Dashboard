import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  BarChart3,
  FileText,
  TrendingUp,
  Brain,
  Plus,
  RefreshCw,
  Zap,
} from 'lucide-react';
import { useCampaigns } from '../hooks/useCampaigns';
import { useActiveProfile } from '../hooks/useActiveProfile';
import { apiFetch } from '../hooks/useApi';
import CampaignCard from '../components/CampaignCard';

interface OverviewStats {
  active_campaigns: number;
  total_posts: number;
  avg_engagement: number;
  avg_ai_score: number;
}

type ToastState = { message: string; type: 'success' | 'error' } | null;

export default function Dashboard() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { activeProfileId, activeProfile } = useActiveProfile();
  const profileParam = activeProfileId ? `?profile_id=${activeProfileId}` : '';
  const { data: campaigns, isLoading } = useCampaigns(activeProfileId);
  const [fetchingMetrics, setFetchingMetrics] = useState(false);
  const [runningAnalysis, setRunningAnalysis] = useState(false);
  const [toast, setToast] = useState<ToastState>(null);

  const { data: overview } = useQuery<OverviewStats>({
    queryKey: ['analytics', 'overview', activeProfileId ?? 'all'],
    queryFn: () => apiFetch<OverviewStats>(`/analytics/overview${profileParam}`),
  });

  const { data: latestInsight } = useQuery<{ insight: string }>({
    queryKey: ['ai', 'cross-campaign'],
    queryFn: () => apiFetch<{ insight: string }>('/ai/cross-campaign-insight'),
  });

  const showToast = (message: string, type: 'success' | 'error') => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 4000);
  };

  const handleFetchMetrics = async () => {
    setFetchingMetrics(true);
    try {
      await apiFetch('/metrics/fetch', { method: 'POST' });
      queryClient.invalidateQueries({ queryKey: ['campaigns'] });
      queryClient.invalidateQueries({ queryKey: ['analytics'] });
      showToast('Metric fetch completed', 'success');
    } catch {
      showToast('Metric fetch failed', 'error');
    } finally {
      setFetchingMetrics(false);
    }
  };

  const handleRunAnalysis = async () => {
    setRunningAnalysis(true);
    try {
      await apiFetch('/ai/analyze', { method: 'POST' });
      queryClient.invalidateQueries({ queryKey: ['ai'] });
      showToast('AI analysis completed', 'success');
    } catch {
      showToast('AI analysis failed', 'error');
    } finally {
      setRunningAnalysis(false);
    }
  };

  return (
    <div>
      <div className="page-header">
        <h2>Dashboard{activeProfile ? ` — ${activeProfile.name}` : ''}</h2>
        <p>Your marketing campaigns at a glance</p>
      </div>

      {/* Stat Cards */}
      <div className="stat-grid">
        <div className="stat-card">
          <div className="stat-icon">
            <BarChart3 size={18} />
          </div>
          <span className="stat-label">Active Campaigns</span>
          <span className="stat-value">
            {isLoading ? (
              <span className="skeleton skeleton-text" style={{ width: 40 }} />
            ) : (
              overview?.active_campaigns ??
                campaigns?.filter((c) => c.status === 'active').length ??
                0
            )}
          </span>
        </div>
        <div className="stat-card">
          <div className="stat-icon">
            <FileText size={18} />
          </div>
          <span className="stat-label">Posts Tracked</span>
          <span className="stat-value">
            {isLoading ? (
              <span className="skeleton skeleton-text" style={{ width: 40 }} />
            ) : (
              overview?.total_posts ??
                campaigns?.reduce(
                  (acc, c) => acc + (c.posts?.length ?? 0),
                  0
                ) ??
                0
            )}
          </span>
        </div>
        <div className="stat-card">
          <div className="stat-icon">
            <TrendingUp size={18} />
          </div>
          <span className="stat-label">Avg Engagement</span>
          <span className="stat-value">
            {isLoading ? (
              <span className="skeleton skeleton-text" style={{ width: 40 }} />
            ) : (
              overview?.avg_engagement?.toFixed(1) ?? '--'
            )}
          </span>
        </div>
        <div className="stat-card">
          <div className="stat-icon">
            <Brain size={18} />
          </div>
          <span className="stat-label">AI Score</span>
          <span className="stat-value">
            {isLoading ? (
              <span className="skeleton skeleton-text" style={{ width: 40 }} />
            ) : (
              overview?.avg_ai_score?.toFixed(1) ?? '--'
            )}
          </span>
        </div>
      </div>

      {/* Quick Actions */}
      <div className="quick-actions">
        <button
          className="btn btn-primary"
          onClick={() => navigate('/campaigns/new')}
        >
          <Plus size={16} /> New Campaign
        </button>
        <button
          className="btn btn-secondary"
          onClick={handleFetchMetrics}
          disabled={fetchingMetrics}
        >
          <RefreshCw size={16} className={fetchingMetrics ? 'spin' : ''} />{' '}
          {fetchingMetrics ? 'Fetching...' : 'Fetch Metrics Now'}
        </button>
        <button
          className="btn btn-secondary"
          onClick={handleRunAnalysis}
          disabled={runningAnalysis}
        >
          <Zap size={16} />{' '}
          {runningAnalysis ? 'Analyzing...' : 'Run AI Analysis'}
        </button>
      </div>

      {toast && (
        <div
          style={{
            padding: '10px 16px',
            borderRadius: 'var(--radius-sm)',
            marginBottom: 16,
            fontSize: '0.875rem',
            background: toast.type === 'success' ? 'var(--success-bg, rgba(34,197,94,0.12))' : 'var(--danger-bg, rgba(239,68,68,0.12))',
            color: toast.type === 'success' ? 'var(--success, #22c55e)' : 'var(--danger, #ef4444)',
            border: `1px solid ${toast.type === 'success' ? 'var(--success, #22c55e)' : 'var(--danger, #ef4444)'}`,
          }}
        >
          {toast.message}
        </div>
      )}

      {/* Campaign Grid */}
      {isLoading ? (
        <div className="campaign-grid">
          {[1, 2, 3, 4, 5, 6].map((i) => (
            <div key={i} className="card">
              <div
                className="skeleton skeleton-title"
                style={{ marginBottom: 12 }}
              />
              <div className="skeleton skeleton-text" />
              <div
                className="skeleton skeleton-text"
                style={{ width: '40%' }}
              />
            </div>
          ))}
        </div>
      ) : campaigns && campaigns.length > 0 ? (
        <div className="campaign-grid">
          {campaigns.map((campaign) => (
            <CampaignCard key={campaign.id} campaign={campaign} />
          ))}
        </div>
      ) : (
        <div className="empty-state">
          <BarChart3 />
          <h3>No campaigns yet</h3>
          <p>
            Create your first campaign to start tracking your marketing
            efforts across platforms.
          </p>
          <button
            className="btn btn-primary"
            onClick={() => navigate('/campaigns/new')}
          >
            <Plus size={16} /> Create Campaign
          </button>
        </div>
      )}

      {/* AI Callout */}
      {latestInsight?.insight && (
        <div className="ai-callout">
          <Brain size={20} />
          <div className="ai-callout-text">
            <div className="ai-callout-label">AI says:</div>
            <div>{latestInsight.insight}</div>
          </div>
        </div>
      )}
    </div>
  );
}
