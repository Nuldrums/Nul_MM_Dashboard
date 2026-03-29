import { useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
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
import { apiFetch } from '../hooks/useApi';
import CampaignCard from '../components/CampaignCard';

interface OverviewStats {
  active_campaigns: number;
  total_posts: number;
  avg_engagement: number;
  avg_ai_score: number;
}

export default function Dashboard() {
  const navigate = useNavigate();
  const { data: campaigns, isLoading } = useCampaigns();

  const { data: overview } = useQuery<OverviewStats>({
    queryKey: ['analytics', 'overview'],
    queryFn: () => apiFetch<OverviewStats>('/analytics/overview'),
  });

  const { data: latestInsight } = useQuery<{ insight: string }>({
    queryKey: ['ai', 'cross-campaign'],
    queryFn: () => apiFetch<{ insight: string }>('/ai/cross-campaign-insight'),
  });

  const handleFetchMetrics = async () => {
    try {
      await apiFetch('/metrics/fetch', { method: 'POST' });
    } catch {
      // silently handle
    }
  };

  const handleRunAnalysis = async () => {
    try {
      await apiFetch('/ai/analyze', { method: 'POST' });
    } catch {
      // silently handle
    }
  };

  return (
    <div>
      <div className="page-header">
        <h2>Dashboard</h2>
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
        <button className="btn btn-secondary" onClick={handleFetchMetrics}>
          <RefreshCw size={16} /> Fetch Metrics Now
        </button>
        <button className="btn btn-secondary" onClick={handleRunAnalysis}>
          <Zap size={16} /> Run AI Analysis
        </button>
      </div>

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
