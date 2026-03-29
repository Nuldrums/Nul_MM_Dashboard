import { useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';
import {
  ArrowLeft,
  Plus,
  TrendingUp,
  Calendar,
  Target,
  X,
  Brain,
} from 'lucide-react';
import { useCampaign, useUpdateCampaign } from '../hooks/useCampaigns';
import { apiFetch } from '../hooks/useApi';
import type { Post, AIAnalysis, Platform, PostType } from '../lib/types';
import { PLATFORM_NAMES } from '../lib/constants';
import PostRow from '../components/PostRow';
import EngagementChart from '../components/EngagementChart';
import AIRecommendation from '../components/AIRecommendation';

const PLATFORMS: Platform[] = [
  'reddit',
  'x',
  'youtube',
  'discord',
  'producthunt',
  'hackernews',
  'tiktok',
  'instagram',
  'linkedin',
  'other',
];

const POST_TYPES: PostType[] = [
  'text',
  'image',
  'video_short',
  'video_long',
  'thread',
  'comment',
  'link',
  'self_promo',
];

const API_TRACKED_PLATFORMS = ['reddit', 'youtube', 'x'];

function detectPlatformFromUrl(url: string): Platform | '' {
  if (!url) return '';
  const lower = url.toLowerCase();
  if (lower.includes('reddit.com')) return 'reddit';
  if (lower.includes('twitter.com') || lower.includes('x.com')) return 'x';
  if (lower.includes('youtube.com') || lower.includes('youtu.be'))
    return 'youtube';
  if (lower.includes('discord.com') || lower.includes('discord.gg'))
    return 'discord';
  if (lower.includes('producthunt.com')) return 'producthunt';
  if (lower.includes('news.ycombinator.com')) return 'hackernews';
  if (lower.includes('tiktok.com')) return 'tiktok';
  if (lower.includes('instagram.com')) return 'instagram';
  if (lower.includes('linkedin.com')) return 'linkedin';
  return '';
}

export default function CampaignDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: campaign, isLoading } = useCampaign(id ?? '');
  const updateCampaign = useUpdateCampaign();

  const [tab, setTab] = useState<
    'posts' | 'metrics' | 'ai' | 'settings'
  >('posts');
  const [showAddPost, setShowAddPost] = useState(false);

  // Add Post form state
  const [postUrl, setPostUrl] = useState('');
  const [postPlatform, setPostPlatform] = useState<string>('');
  const [postType, setPostType] = useState<string>('text');
  const [postTitle, setPostTitle] = useState('');
  const [postCommunity, setPostCommunity] = useState('');
  const [postPlatformId, setPostPlatformId] = useState('');

  // Settings form state
  const [editName, setEditName] = useState('');
  const [editGoal, setEditGoal] = useState('');
  const [editAudience, setEditAudience] = useState('');
  const [settingsInit, setSettingsInit] = useState(false);

  // Fetch AI analysis
  const { data: analysis } = useQuery<AIAnalysis>({
    queryKey: ['ai', 'campaign', id],
    queryFn: () => apiFetch<AIAnalysis>(`/ai/campaigns/${id}/latest`),
    enabled: tab === 'ai' && !!id,
  });

  // Fetch campaign metrics for charts
  const { data: metricsTimeline } = useQuery<
    { date: string; value: number }[]
  >({
    queryKey: ['metrics', 'timeline', id],
    queryFn: () =>
      apiFetch<{ date: string; value: number }[]>(
        `/campaigns/${id}/metrics/timeline`
      ),
    enabled: tab === 'metrics' && !!id,
  });

  const { data: platformBreakdown } = useQuery<
    { platform: string; engagement: number }[]
  >({
    queryKey: ['metrics', 'platform-breakdown', id],
    queryFn: () =>
      apiFetch<{ platform: string; engagement: number }[]>(
        `/campaigns/${id}/metrics/platforms`
      ),
    enabled: tab === 'metrics' && !!id,
  });

  const { data: postTypeBreakdown } = useQuery<
    { post_type: string; engagement: number }[]
  >({
    queryKey: ['metrics', 'posttype-breakdown', id],
    queryFn: () =>
      apiFetch<{ post_type: string; engagement: number }[]>(
        `/campaigns/${id}/metrics/post-types`
      ),
    enabled: tab === 'metrics' && !!id,
  });

  const addPostMutation = useMutation({
    mutationFn: (data: Partial<Post>) =>
      apiFetch<Post>(`/campaigns/${id}/posts`, {
        method: 'POST',
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['campaigns', id] });
      setShowAddPost(false);
      resetPostForm();
    },
  });

  const resetPostForm = () => {
    setPostUrl('');
    setPostPlatform('');
    setPostType('text');
    setPostTitle('');
    setPostCommunity('');
    setPostPlatformId('');
  };

  const handleUrlChange = (url: string) => {
    setPostUrl(url);
    const detected = detectPlatformFromUrl(url);
    if (detected) setPostPlatform(detected);
  };

  const handleAddPost = (e: React.FormEvent) => {
    e.preventDefault();
    const platform = postPlatform || 'other';
    addPostMutation.mutate({
      platform,
      post_type: postType,
      url: postUrl || undefined,
      title: postTitle || undefined,
      target_community: postCommunity || undefined,
      platform_post_id: postPlatformId || undefined,
      is_api_tracked: API_TRACKED_PLATFORMS.includes(platform),
      tags: [],
    });
  };

  // Init settings tab
  if (campaign && !settingsInit) {
    setEditName(campaign.name);
    setEditGoal(campaign.goal ?? '');
    setEditAudience(campaign.target_audience ?? '');
    setSettingsInit(true);
  }

  const handleSaveSettings = () => {
    if (!id) return;
    updateCampaign.mutate({
      id,
      name: editName,
      goal: editGoal || undefined,
      target_audience: editAudience || undefined,
    });
  };

  const handleArchive = () => {
    if (!id || !confirm('Are you sure you want to archive this campaign?'))
      return;
    updateCampaign.mutate(
      { id, status: 'archived' },
      { onSuccess: () => navigate('/') }
    );
  };

  if (isLoading) {
    return (
      <div>
        <div className="skeleton skeleton-title" style={{ width: 200 }} />
        <div className="skeleton skeleton-text" />
        <div className="skeleton skeleton-text" style={{ width: '60%' }} />
        <div
          className="skeleton skeleton-card"
          style={{ marginTop: 24 }}
        />
      </div>
    );
  }

  if (!campaign) {
    return (
      <div className="empty-state">
        <h3>Campaign not found</h3>
        <button className="btn btn-primary" onClick={() => navigate('/')}>
          Back to Dashboard
        </button>
      </div>
    );
  }

  const score = campaign.metrics_summary?.ai_score;
  const scoreClass =
    score != null
      ? score >= 7
        ? 'score-high'
        : score >= 4
          ? 'score-mid'
          : 'score-low'
      : '';

  return (
    <div>
      {/* Header */}
      <button
        className="btn btn-ghost"
        onClick={() => navigate('/')}
        style={{ marginBottom: 12 }}
      >
        <ArrowLeft size={16} /> Dashboard
      </button>

      <div className="flex-between" style={{ marginBottom: 8 }}>
        <div>
          <h2 style={{ fontSize: '1.5rem', fontWeight: 700, margin: 0 }}>
            {campaign.name}
          </h2>
          <div
            className="flex-gap"
            style={{ marginTop: 6, gap: 12, flexWrap: 'wrap' }}
          >
            <span className="text-muted">
              {campaign.product?.name ?? 'No product'}
            </span>
            <span className={`badge badge-${campaign.status}`}>
              {campaign.status}
            </span>
            {campaign.start_date && (
              <span className="flex-gap text-muted">
                <Calendar size={12} />
                {new Date(campaign.start_date).toLocaleDateString()}
                {campaign.end_date &&
                  ` - ${new Date(campaign.end_date).toLocaleDateString()}`}
              </span>
            )}
            {campaign.goal && (
              <span className="flex-gap text-muted">
                <Target size={12} />
                {campaign.goal.replace(/_/g, ' ')}
              </span>
            )}
          </div>
        </div>
        {score != null && (
          <div style={{ textAlign: 'right' }}>
            <div className="text-muted" style={{ marginBottom: 4 }}>
              AI Score
            </div>
            <div
              className={scoreClass}
              style={{ fontSize: '2rem', fontWeight: 700, lineHeight: 1 }}
            >
              <TrendingUp
                size={20}
                style={{ verticalAlign: 'middle', marginRight: 4 }}
              />
              {score.toFixed(1)}
            </div>
          </div>
        )}
      </div>

      {/* Tab Bar */}
      <div className="tab-bar" style={{ marginTop: 20 }}>
        {(['posts', 'metrics', 'ai', 'settings'] as const).map((t) => (
          <button
            key={t}
            className={tab === t ? 'active' : ''}
            onClick={() => setTab(t)}
          >
            {t === 'ai'
              ? 'AI Insights'
              : t.charAt(0).toUpperCase() + t.slice(1)}
          </button>
        ))}
      </div>

      {/* Posts Tab */}
      {tab === 'posts' && (
        <div>
          <div className="flex-between mb-16">
            <span className="text-muted">
              {campaign.posts?.length ?? 0} post
              {(campaign.posts?.length ?? 0) !== 1 ? 's' : ''}
            </span>
            <button
              className="btn btn-primary btn-sm"
              onClick={() => setShowAddPost(true)}
            >
              <Plus size={14} /> Add Post
            </button>
          </div>

          {campaign.posts && campaign.posts.length > 0 ? (
            <div className="table-wrapper">
              <table>
                <thead>
                  <tr>
                    <th style={{ width: 28 }} />
                    <th>Platform</th>
                    <th>Title / Preview</th>
                    <th>Community</th>
                    <th>Posted</th>
                    <th style={{ textAlign: 'right' }}>Engagement</th>
                  </tr>
                </thead>
                <tbody>
                  {campaign.posts.map((post) => (
                    <PostRow key={post.id} post={post} />
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="empty-state">
              <h3>No posts yet</h3>
              <p>Add your first post to start tracking engagement.</p>
              <button
                className="btn btn-primary"
                onClick={() => setShowAddPost(true)}
              >
                <Plus size={16} /> Add Post
              </button>
            </div>
          )}

          {/* Add Post Modal */}
          {showAddPost && (
            <div className="modal-overlay" onClick={() => setShowAddPost(false)}>
              <div className="modal" onClick={(e) => e.stopPropagation()}>
                <div className="flex-between" style={{ marginBottom: 16 }}>
                  <h3 style={{ margin: 0 }}>Add Post</h3>
                  <button
                    className="btn btn-ghost btn-sm"
                    onClick={() => setShowAddPost(false)}
                  >
                    <X size={16} />
                  </button>
                </div>
                <form onSubmit={handleAddPost}>
                  <div className="form-group">
                    <label>URL (auto-detects platform)</label>
                    <input
                      className="form-input"
                      type="url"
                      value={postUrl}
                      onChange={(e) => handleUrlChange(e.target.value)}
                      placeholder="https://reddit.com/r/..."
                    />
                  </div>
                  <div className="form-row">
                    <div className="form-group">
                      <label>Platform *</label>
                      <select
                        className="form-select"
                        value={postPlatform}
                        onChange={(e) => setPostPlatform(e.target.value)}
                        required
                      >
                        <option value="">Select...</option>
                        {PLATFORMS.map((p) => (
                          <option key={p} value={p}>
                            {PLATFORM_NAMES[p] ?? p}
                          </option>
                        ))}
                      </select>
                    </div>
                    <div className="form-group">
                      <label>Post Type</label>
                      <select
                        className="form-select"
                        value={postType}
                        onChange={(e) => setPostType(e.target.value)}
                      >
                        {POST_TYPES.map((t) => (
                          <option key={t} value={t}>
                            {t.replace(/_/g, ' ')}
                          </option>
                        ))}
                      </select>
                    </div>
                  </div>
                  <div className="form-group">
                    <label>Title</label>
                    <input
                      className="form-input"
                      type="text"
                      value={postTitle}
                      onChange={(e) => setPostTitle(e.target.value)}
                      placeholder="Post title or preview text"
                    />
                  </div>
                  <div className="form-row">
                    <div className="form-group">
                      <label>Community / Channel</label>
                      <input
                        className="form-input"
                        type="text"
                        value={postCommunity}
                        onChange={(e) => setPostCommunity(e.target.value)}
                        placeholder="e.g., r/SideProject"
                      />
                    </div>
                    <div className="form-group">
                      <label>Platform Post ID</label>
                      <input
                        className="form-input"
                        type="text"
                        value={postPlatformId}
                        onChange={(e) => setPostPlatformId(e.target.value)}
                        placeholder="Optional"
                      />
                    </div>
                  </div>
                  {postPlatform && (
                    <p className="text-muted" style={{ marginBottom: 12 }}>
                      API tracking:{' '}
                      {API_TRACKED_PLATFORMS.includes(postPlatform)
                        ? 'Enabled (metrics will be fetched automatically)'
                        : 'Manual only (enter metrics by hand)'}
                    </p>
                  )}
                  <div style={{ display: 'flex', gap: 10 }}>
                    <button
                      type="submit"
                      className="btn btn-primary"
                      disabled={
                        addPostMutation.isPending || !postPlatform
                      }
                    >
                      {addPostMutation.isPending
                        ? 'Adding...'
                        : 'Add Post'}
                    </button>
                    <button
                      type="button"
                      className="btn btn-secondary"
                      onClick={() => setShowAddPost(false)}
                    >
                      Cancel
                    </button>
                  </div>
                </form>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Metrics Tab */}
      {tab === 'metrics' && (
        <div>
          {metricsTimeline && metricsTimeline.length > 0 ? (
            <EngagementChart
              data={metricsTimeline}
              title="Engagement Over Time"
            />
          ) : (
            <div className="card mb-24">
              <div className="empty-state">
                <TrendingUp />
                <p>
                  No timeline data yet. Fetch metrics to see engagement
                  trends.
                </p>
              </div>
            </div>
          )}

          <div
            style={{
              display: 'grid',
              gridTemplateColumns: '1fr 1fr',
              gap: 16,
              marginTop: 16,
            }}
          >
            <div className="card">
              <h3
                style={{
                  fontSize: '0.95rem',
                  fontWeight: 600,
                  marginBottom: 16,
                }}
              >
                By Platform
              </h3>
              {platformBreakdown && platformBreakdown.length > 0 ? (
                <div style={{ width: '100%', height: 220 }}>
                  <ResponsiveContainer width="100%" height="100%">
                    <BarChart
                      data={platformBreakdown.map((p) => ({
                        ...p,
                        name:
                          PLATFORM_NAMES[p.platform] ?? p.platform,
                      }))}
                    >
                      <CartesianGrid
                        strokeDasharray="3 3"
                        stroke="var(--border-light)"
                      />
                      <XAxis
                        dataKey="name"
                        tick={{
                          fill: 'var(--text-tertiary)',
                          fontSize: 10,
                        }}
                      />
                      <YAxis
                        tick={{
                          fill: 'var(--text-tertiary)',
                          fontSize: 10,
                        }}
                        axisLine={false}
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
                        fill="var(--chart-1)"
                        radius={[4, 4, 0, 0]}
                      />
                    </BarChart>
                  </ResponsiveContainer>
                </div>
              ) : (
                <p className="text-muted">No data yet</p>
              )}
            </div>

            <div className="card">
              <h3
                style={{
                  fontSize: '0.95rem',
                  fontWeight: 600,
                  marginBottom: 16,
                }}
              >
                By Post Type
              </h3>
              {postTypeBreakdown && postTypeBreakdown.length > 0 ? (
                <div style={{ width: '100%', height: 220 }}>
                  <ResponsiveContainer width="100%" height="100%">
                    <BarChart
                      data={postTypeBreakdown.map((p) => ({
                        ...p,
                        name: p.post_type.replace(/_/g, ' '),
                      }))}
                    >
                      <CartesianGrid
                        strokeDasharray="3 3"
                        stroke="var(--border-light)"
                      />
                      <XAxis
                        dataKey="name"
                        tick={{
                          fill: 'var(--text-tertiary)',
                          fontSize: 10,
                        }}
                      />
                      <YAxis
                        tick={{
                          fill: 'var(--text-tertiary)',
                          fontSize: 10,
                        }}
                        axisLine={false}
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
                        fill="var(--chart-2)"
                        radius={[4, 4, 0, 0]}
                      />
                    </BarChart>
                  </ResponsiveContainer>
                </div>
              ) : (
                <p className="text-muted">No data yet</p>
              )}
            </div>
          </div>
        </div>
      )}

      {/* AI Insights Tab */}
      {tab === 'ai' && (
        <div>
          {analysis ? (
            <>
              <div className="card mb-24">
                <h3
                  style={{
                    fontSize: '0.95rem',
                    fontWeight: 600,
                    marginBottom: 12,
                  }}
                >
                  Summary
                </h3>
                <p style={{ fontSize: '0.875rem', lineHeight: 1.6 }}>
                  {analysis.summary}
                </p>
                <span className="text-muted" style={{ marginTop: 8, display: 'block' }}>
                  Analyzed {new Date(analysis.analyzed_at).toLocaleString()}
                  {analysis.model_used && ` using ${analysis.model_used}`}
                </span>
              </div>

              {analysis.top_performers &&
                analysis.top_performers.length > 0 && (
                  <div className="section">
                    <h3 className="section-title">Top Performers</h3>
                    <div
                      style={{
                        display: 'flex',
                        flexDirection: 'column',
                        gap: 8,
                      }}
                    >
                      {analysis.top_performers.map((tp, i) => (
                        <div key={i} className="card">
                          <div className="flex-between">
                            <span style={{ fontWeight: 600, fontSize: '0.875rem' }}>
                              Post: {tp.post_id.slice(0, 8)}...
                            </span>
                            <span className="score-high" style={{ fontWeight: 700 }}>
                              {tp.score.toFixed(1)}
                            </span>
                          </div>
                          <p className="text-muted" style={{ marginTop: 4 }}>
                            {tp.reasoning}
                          </p>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

              {analysis.underperformers &&
                analysis.underperformers.length > 0 && (
                  <div className="section">
                    <h3 className="section-title">Underperformers</h3>
                    <div
                      style={{
                        display: 'flex',
                        flexDirection: 'column',
                        gap: 8,
                      }}
                    >
                      {analysis.underperformers.map((up, i) => (
                        <div key={i} className="card">
                          <div className="flex-between">
                            <span style={{ fontWeight: 600, fontSize: '0.875rem' }}>
                              Post: {up.post_id.slice(0, 8)}...
                            </span>
                            <span className="score-low" style={{ fontWeight: 700 }}>
                              {up.score.toFixed(1)}
                            </span>
                          </div>
                          <p className="text-muted" style={{ marginTop: 4 }}>
                            {up.reasoning}
                          </p>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

              {analysis.patterns && analysis.patterns.length > 0 && (
                <div className="section">
                  <h3 className="section-title">Patterns Detected</h3>
                  <div
                    style={{
                      display: 'flex',
                      flexDirection: 'column',
                      gap: 8,
                    }}
                  >
                    {analysis.patterns.map((pat, i) => (
                      <div key={i} className="card">
                        <div
                          className="flex-between"
                          style={{ marginBottom: 6 }}
                        >
                          <span style={{ fontWeight: 600, fontSize: '0.875rem' }}>
                            {pat.pattern}
                          </span>
                          <span
                            className="confidence-badge"
                            style={{
                              background:
                                pat.confidence === 'high'
                                  ? 'var(--success-bg)'
                                  : pat.confidence === 'medium'
                                    ? 'var(--warning-bg)'
                                    : 'var(--bg-tertiary)',
                              color:
                                pat.confidence === 'high'
                                  ? 'var(--success)'
                                  : pat.confidence === 'medium'
                                    ? 'var(--warning)'
                                    : 'var(--text-tertiary)',
                            }}
                          >
                            {pat.confidence}
                          </span>
                        </div>
                        <p
                          className="text-muted"
                          style={{ margin: 0, fontSize: '0.825rem' }}
                        >
                          {pat.evidence}
                        </p>
                        {pat.actionable_insight && (
                          <p
                            style={{
                              margin: '6px 0 0',
                              fontSize: '0.825rem',
                              color: 'var(--accent-primary)',
                            }}
                          >
                            {pat.actionable_insight}
                          </p>
                        )}
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {analysis.recommendations &&
                analysis.recommendations.length > 0 && (
                  <div className="section">
                    <h3 className="section-title">Recommendations</h3>
                    <div
                      style={{
                        display: 'flex',
                        flexDirection: 'column',
                        gap: 12,
                      }}
                    >
                      {analysis.recommendations.map((rec, i) => (
                        <AIRecommendation
                          key={i}
                          recommendation={rec}
                        />
                      ))}
                    </div>
                  </div>
                )}
            </>
          ) : (
            <div className="empty-state">
              <Brain size={48} style={{ opacity: 0.5 }} />
              <h3>No AI analysis yet</h3>
              <p>
                Run an AI analysis from the dashboard to get insights about
                this campaign.
              </p>
            </div>
          )}
        </div>
      )}

      {/* Settings Tab */}
      {tab === 'settings' && (
        <div style={{ maxWidth: 520 }}>
          <div className="card mb-24">
            <h3
              style={{
                fontSize: '0.95rem',
                fontWeight: 600,
                marginBottom: 16,
              }}
            >
              Campaign Settings
            </h3>
            <div className="form-group">
              <label>Campaign Name</label>
              <input
                className="form-input"
                type="text"
                value={editName}
                onChange={(e) => setEditName(e.target.value)}
              />
            </div>
            <div className="form-group">
              <label>Goal</label>
              <select
                className="form-select"
                value={editGoal}
                onChange={(e) => setEditGoal(e.target.value)}
              >
                <option value="">No goal set</option>
                <option value="drive_sales">Drive Sales</option>
                <option value="awareness">Brand Awareness</option>
                <option value="traffic">Drive Traffic</option>
                <option value="community_growth">
                  Community Growth
                </option>
              </select>
            </div>
            <div className="form-group">
              <label>Target Audience</label>
              <input
                className="form-input"
                type="text"
                value={editAudience}
                onChange={(e) => setEditAudience(e.target.value)}
              />
            </div>
            <div style={{ display: 'flex', gap: 10 }}>
              <button
                className="btn btn-primary"
                onClick={handleSaveSettings}
                disabled={updateCampaign.isPending}
              >
                {updateCampaign.isPending ? 'Saving...' : 'Save Changes'}
              </button>
            </div>
          </div>

          <div className="card">
            <h3
              style={{
                fontSize: '0.95rem',
                fontWeight: 600,
                marginBottom: 12,
                color: 'var(--danger)',
              }}
            >
              Danger Zone
            </h3>
            <p className="text-muted" style={{ marginBottom: 12 }}>
              Archiving a campaign hides it from the dashboard and stops
              metric collection.
            </p>
            <button className="btn btn-danger" onClick={handleArchive}>
              Archive Campaign
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
