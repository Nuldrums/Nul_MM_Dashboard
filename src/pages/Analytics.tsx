import { useQuery } from '@tanstack/react-query';
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';
import { BarChart3, FileText, TrendingUp } from 'lucide-react';
import { apiFetch } from '../hooks/useApi';
import { useCampaigns } from '../hooks/useCampaigns';
import { PLATFORM_COLORS, PLATFORM_NAMES } from '../lib/constants';
import EngagementChart from '../components/EngagementChart';

interface PlatformBreakdown {
  platform: string;
  engagement: number;
  posts: number;
}

interface PostTypeBreakdown {
  post_type: string;
  engagement: number;
  count: number;
}

interface TrendPoint {
  date: string;
  value: number;
}

export default function Analytics() {
  const { data: campaigns } = useCampaigns();

  const { data: platformData } = useQuery<PlatformBreakdown[]>({
    queryKey: ['analytics', 'platforms'],
    queryFn: () => apiFetch<PlatformBreakdown[]>('/analytics/platforms'),
  });

  const { data: postTypeData } = useQuery<PostTypeBreakdown[]>({
    queryKey: ['analytics', 'post-types'],
    queryFn: () => apiFetch<PostTypeBreakdown[]>('/analytics/post-types'),
  });

  const { data: trendData } = useQuery<TrendPoint[]>({
    queryKey: ['analytics', 'trends'],
    queryFn: () => apiFetch<TrendPoint[]>('/analytics/trends'),
  });

  const totalCampaigns = campaigns?.length ?? 0;
  const totalPosts =
    campaigns?.reduce((acc, c) => acc + (c.posts?.length ?? 0), 0) ?? 0;
  const totalEngagement =
    campaigns?.reduce(
      (acc, c) => acc + (c.metrics_summary?.total_views ?? 0),
      0
    ) ?? 0;

  const platformChartData = (platformData ?? []).map((p) => ({
    ...p,
    name: PLATFORM_NAMES[p.platform] ?? p.platform,
    fill: PLATFORM_COLORS[p.platform] ?? PLATFORM_COLORS.other,
  }));

  const postTypeChartData = (postTypeData ?? []).map((p) => ({
    ...p,
    name: p.post_type.replace(/_/g, ' '),
  }));

  return (
    <div>
      <div className="page-header">
        <h2>Analytics</h2>
        <p>Cross-campaign performance metrics</p>
      </div>

      {/* Overview Stats */}
      <div className="stat-grid">
        <div className="stat-card">
          <div className="stat-icon">
            <BarChart3 size={18} />
          </div>
          <span className="stat-label">Total Campaigns</span>
          <span className="stat-value">{totalCampaigns}</span>
        </div>
        <div className="stat-card">
          <div className="stat-icon">
            <FileText size={18} />
          </div>
          <span className="stat-label">Total Posts</span>
          <span className="stat-value">{totalPosts}</span>
        </div>
        <div className="stat-card">
          <div className="stat-icon">
            <TrendingUp size={18} />
          </div>
          <span className="stat-label">Total Engagement</span>
          <span className="stat-value">
            {totalEngagement.toLocaleString()}
          </span>
        </div>
      </div>

      {/* Platform Breakdown */}
      <div className="section">
        <h3 className="section-title">Engagement by Platform</h3>
        <div className="card">
          {platformChartData.length > 0 ? (
            <div style={{ width: '100%', height: 300 }}>
              <ResponsiveContainer width="100%" height="100%">
                <BarChart
                  data={platformChartData}
                  layout="vertical"
                  margin={{ top: 0, right: 20, left: 60, bottom: 0 }}
                >
                  <CartesianGrid
                    strokeDasharray="3 3"
                    stroke="var(--border-light)"
                  />
                  <XAxis
                    type="number"
                    tick={{
                      fill: 'var(--text-tertiary)',
                      fontSize: 11,
                    }}
                    axisLine={{ stroke: 'var(--border-light)' }}
                  />
                  <YAxis
                    type="category"
                    dataKey="name"
                    tick={{
                      fill: 'var(--text-secondary)',
                      fontSize: 12,
                    }}
                    axisLine={false}
                    tickLine={false}
                    width={80}
                  />
                  <Tooltip
                    contentStyle={{
                      background: 'var(--surface-card)',
                      border: '1px solid var(--border-medium)',
                      borderRadius: 'var(--radius-sm)',
                      fontSize: '0.8rem',
                    }}
                  />
                  <Bar
                    dataKey="engagement"
                    name="Engagement"
                    fill="var(--chart-1)"
                    radius={[0, 4, 4, 0]}
                  />
                </BarChart>
              </ResponsiveContainer>
            </div>
          ) : (
            <div className="empty-state">
              <BarChart3 />
              <p>
                No platform data yet. Add posts to your campaigns to see
                platform breakdowns.
              </p>
            </div>
          )}
        </div>
      </div>

      {/* Post Type Breakdown */}
      <div className="section">
        <h3 className="section-title">Engagement by Post Type</h3>
        <div className="card">
          {postTypeChartData.length > 0 ? (
            <div style={{ width: '100%', height: 300 }}>
              <ResponsiveContainer width="100%" height="100%">
                <BarChart
                  data={postTypeChartData}
                  margin={{ top: 0, right: 20, left: -12, bottom: 0 }}
                >
                  <CartesianGrid
                    strokeDasharray="3 3"
                    stroke="var(--border-light)"
                  />
                  <XAxis
                    dataKey="name"
                    tick={{
                      fill: 'var(--text-secondary)',
                      fontSize: 11,
                    }}
                    axisLine={{ stroke: 'var(--border-light)' }}
                  />
                  <YAxis
                    tick={{
                      fill: 'var(--text-tertiary)',
                      fontSize: 11,
                    }}
                    axisLine={false}
                    tickLine={false}
                  />
                  <Tooltip
                    contentStyle={{
                      background: 'var(--surface-card)',
                      border: '1px solid var(--border-medium)',
                      borderRadius: 'var(--radius-sm)',
                      fontSize: '0.8rem',
                    }}
                  />
                  <Bar
                    dataKey="engagement"
                    name="Engagement"
                    fill="var(--chart-2)"
                    radius={[4, 4, 0, 0]}
                  />
                </BarChart>
              </ResponsiveContainer>
            </div>
          ) : (
            <div className="empty-state">
              <BarChart3 />
              <p>No post type data available yet.</p>
            </div>
          )}
        </div>
      </div>

      {/* Trends */}
      <div className="section">
        <h3 className="section-title">Engagement Trends</h3>
        {trendData && trendData.length > 0 ? (
          <EngagementChart data={trendData} title="Overall Engagement" />
        ) : (
          <div className="card">
            <div className="empty-state">
              <TrendingUp />
              <p>
                Trend data will appear here once metrics are collected over
                time.
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
