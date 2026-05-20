import { useState, useMemo } from 'react';
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
  ChevronUp,
  ChevronDown,
  ChevronsUpDown,
} from 'lucide-react';
import { Link } from 'react-router-dom';
import { useCampaign, useUpdateCampaign, useDeleteCampaign } from '../hooks/useCampaigns';
import { useFeeds, useCreateFeed, useDeleteFeed } from '../hooks/useFeeds';
import { useAccounts } from '../hooks/useAccounts';
import { apiFetch } from '../hooks/useApi';
import type { Post, AIAnalysis, Platform, PostType } from '../lib/types';
import { PLATFORM_NAMES, PLATFORM_COLORS } from '../lib/constants';

function formatCount(n: number): string {
  // Show full number with thousands separators below 10K; abbreviate above.
  if (n < 10_000) return n.toLocaleString();
  if (n < 1_000_000) return `${(n / 1000).toFixed(n < 100_000 ? 1 : 0)}K`;
  if (n < 1_000_000_000) return `${(n / 1_000_000).toFixed(n < 10_000_000 ? 1 : 0)}M`;
  return `${(n / 1_000_000_000).toFixed(1)}B`;
}
import PostRow from '../components/PostRow';
import EngagementChart from '../components/EngagementChart';
import AIRecommendation from '../components/AIRecommendation';

const FEED_CONTENT_TYPES: Record<string, { value: string; label: string }[]> = {
  youtube: [
    { value: 'long_form', label: 'Long-form videos' },
    { value: 'short_form', label: 'Shorts (≤3 min)' },
    { value: 'live', label: 'Live streams (completed)' },
  ],
  x: [
    { value: 'tweets', label: 'Original tweets' },
    { value: 'replies', label: 'Replies' },
    { value: 'media', label: 'Media tweets' },
    { value: 'broadcasts', label: 'Live broadcasts' },
  ],
  tiktok: [{ value: 'videos', label: 'Videos' }],
};

const PLATFORMS: Platform[] = [
  'reddit',
  'x',
  'youtube',
  'discord',
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

const API_TRACKED_PLATFORMS = ['reddit', 'youtube', 'x', 'tiktok'];

function detectPlatformFromUrl(url: string): Platform | '' {
  if (!url) return '';
  const lower = url.toLowerCase();
  if (lower.includes('reddit.com')) return 'reddit';
  if (lower.includes('twitter.com') || lower.includes('x.com')) return 'x';
  if (lower.includes('youtube.com') || lower.includes('youtu.be'))
    return 'youtube';
  if (lower.includes('discord.com') || lower.includes('discord.gg'))
    return 'discord';
  if (lower.includes('tiktok.com')) return 'tiktok';
  if (lower.includes('instagram.com')) return 'instagram';
  if (lower.includes('linkedin.com')) return 'linkedin';
  return '';
}

function extractPlatformPostId(url: string, platform: string): string {
  if (!url) return '';
  try {
    const u = new URL(url);
    const path = u.pathname;
    switch (platform) {
      case 'youtube': {
        if (u.hostname.includes('youtu.be')) return path.replace(/^\//, '').split('/')[0] ?? '';
        const m = path.match(/^\/shorts\/([^/?]+)/);
        if (m) return m[1];
        const v = u.searchParams.get('v');
        if (v) return v;
        const embed = path.match(/^\/embed\/([^/?]+)/);
        if (embed) return embed[1];
        return '';
      }
      case 'reddit': {
        const m = path.match(/\/comments\/([a-z0-9]+)/i);
        return m ? m[1] : '';
      }
      case 'x':
      case 'twitter': {
        const m = path.match(/\/status\/(\d+)/);
        return m ? m[1] : '';
      }
      default:
        return '';
    }
  } catch {
    return '';
  }
}

export default function CampaignDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: campaign, isLoading } = useCampaign(id ?? '');
  const updateCampaign = useUpdateCampaign();
  const deleteCampaign = useDeleteCampaign();
  const [confirmPermanentDelete, setConfirmPermanentDelete] = useState(false);

  const [tab, setTab] = useState<
    'posts' | 'metrics' | 'ai' | 'settings'
  >('posts');

  type SortKey = 'posted_at' | 'community' | 'views' | 'likes' | 'comments';
  type SortDir = 'asc' | 'desc';
  const [sortKey, setSortKey] = useState<SortKey>('posted_at');
  const [sortDir, setSortDir] = useState<SortDir>('desc');

  const toggleSort = (key: SortKey) => {
    if (sortKey === key) {
      setSortDir(d => (d === 'asc' ? 'desc' : 'asc'));
    } else {
      setSortKey(key);
      // Sensible defaults per column type: dates and numbers descend, strings ascend.
      setSortDir(key === 'community' ? 'asc' : 'desc');
    }
  };

  const sortedPosts = useMemo(() => {
    const posts = [...(campaign?.posts ?? [])];
    posts.sort((a, b) => {
      let cmp = 0;
      if (sortKey === 'community') {
        const av = (a.target_community ?? '').toLowerCase();
        const bv = (b.target_community ?? '').toLowerCase();
        // Empty values always sort to the end, regardless of direction
        if (!av && !bv) cmp = 0;
        else if (!av) return 1;
        else if (!bv) return -1;
        else cmp = av.localeCompare(bv);
      } else if (sortKey === 'posted_at') {
        const av = a.posted_at ? new Date(a.posted_at).getTime() : 0;
        const bv = b.posted_at ? new Date(b.posted_at).getTime() : 0;
        cmp = av - bv;
      } else {
        const av = a[sortKey] ?? 0;
        const bv = b[sortKey] ?? 0;
        cmp = av - bv;
      }
      return sortDir === 'asc' ? cmp : -cmp;
    });
    return posts;
  }, [campaign?.posts, sortKey, sortDir]);

  const SortHeader = ({ label, sortKey: key, align }: { label: string; sortKey: SortKey; align?: 'left' | 'right' }) => {
    const active = sortKey === key;
    return (
      <th
        style={{
          cursor: 'pointer',
          userSelect: 'none',
          textAlign: align ?? 'left',
        }}
        onClick={() => toggleSort(key)}
      >
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4, justifyContent: align === 'right' ? 'flex-end' : 'flex-start', width: '100%' }}>
          {label}
          {active
            ? (sortDir === 'asc' ? <ChevronUp size={12} /> : <ChevronDown size={12} />)
            : <ChevronsUpDown size={12} style={{ opacity: 0.35 }} />}
        </span>
      </th>
    );
  };
  const [showAddPost, setShowAddPost] = useState(false);
  const [showAddFeed, setShowAddFeed] = useState(false);

  // Add Post form state
  const [postUrl, setPostUrl] = useState('');
  const [postPlatform, setPostPlatform] = useState<string>('');
  const [postType, setPostType] = useState<string>('text');
  const [postTitle, setPostTitle] = useState('');
  const [postCommunity, setPostCommunity] = useState('');
  const [postPlatformId, setPostPlatformId] = useState('');

  // Add Feed form state
  const [feedAccountId, setFeedAccountId] = useState<string>('');
  const [feedContentType, setFeedContentType] = useState<string>('long_form');

  // Feeds for this campaign
  const { data: feeds = [] } = useFeeds(id ?? '');
  const createFeed = useCreateFeed(id ?? '');
  const deleteFeed = useDeleteFeed(id ?? '');

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
      queryClient.invalidateQueries({ queryKey: ['campaigns'] });
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
    if (detected) {
      setPostPlatform(detected);
      const extractedId = extractPlatformPostId(url, detected);
      if (extractedId) setPostPlatformId(extractedId);
    }
  };

  // Connected accounts for the campaign's profile — filtered to feed-capable platforms.
  const { data: profileAccounts = [] } = useAccounts(campaign?.profile_id ?? null);
  const feedCapableAccounts = profileAccounts.filter((a) =>
    ['youtube', 'x', 'tiktok'].includes(a.platform)
  );

  const selectedAccount = feedCapableAccounts.find((a) => a.id === feedAccountId);

  const handleAddFeed = (e: React.FormEvent) => {
    e.preventDefault();
    if (!feedAccountId || !feedContentType) return;
    createFeed.mutate(
      {
        profile_account_id: feedAccountId,
        content_type: feedContentType,
      },
      {
        onSuccess: () => {
          setShowAddFeed(false);
          setFeedAccountId('');
        },
      }
    );
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
    setEditAudience((campaign.target_audience ?? []).join(', '));
    setSettingsInit(true);
  }

  const handleSaveSettings = () => {
    if (!id) return;
    updateCampaign.mutate({
      id,
      name: editName,
      goal: editGoal || undefined,
      target_audience: editAudience
        ? editAudience.split(',').map((s) => s.trim()).filter(Boolean)
        : undefined,
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

  const handlePermanentDelete = () => {
    if (!id) return;
    deleteCampaign.mutate(
      { id, permanent: true },
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
          <div style={{ display: 'flex', alignItems: 'center', gap: 14, flexWrap: 'wrap' }}>
            <h2 style={{ fontSize: '1.5rem', fontWeight: 700, margin: 0 }}>
              {campaign.name}
            </h2>
            {(() => {
              // One pill per unique account (a profile_account can back multiple feeds —
              // e.g. YouTube long-form + shorts — but its follower count is the same).
              const seen = new Set<string>();
              return feeds
                .filter(f => {
                  if (seen.has(f.profile_account_id)) return false;
                  seen.add(f.profile_account_id);
                  return f.follower_count != null;
                })
                .map(f => (
                  <span
                    key={f.profile_account_id}
                    title={`${PLATFORM_NAMES[f.platform] ?? f.platform} · ${f.account_handle}${f.follower_count_at ? ` (updated ${new Date(f.follower_count_at).toLocaleString()})` : ''}`}
                    style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: 6,
                      padding: '3px 10px',
                      borderRadius: 999,
                      background: 'var(--bg-tertiary)',
                      fontSize: '0.85rem',
                    }}
                  >
                    <span
                      style={{
                        width: 8,
                        height: 8,
                        borderRadius: '50%',
                        backgroundColor: PLATFORM_COLORS[f.platform] ?? PLATFORM_COLORS.other,
                      }}
                    />
                    <span style={{ fontWeight: 600 }}>{formatCount(f.follower_count!)}</span>
                  </span>
                ));
            })()}
          </div>
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
            <div style={{ display: 'flex', gap: 8 }}>
              <button
                className="btn btn-secondary btn-sm"
                onClick={() => setShowAddFeed(true)}
                title="Auto-populate new posts from a connected account"
              >
                <Plus size={14} /> Add Feed
              </button>
              <button
                className="btn btn-primary btn-sm"
                onClick={() => setShowAddPost(true)}
              >
                <Plus size={14} /> Add Post
              </button>
            </div>
          </div>

          {feeds.length > 0 && (
            <div className="card mb-16" style={{ padding: 12 }}>
              <div className="text-muted" style={{ fontSize: '0.8rem', marginBottom: 8 }}>
                Auto-feeds
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                {feeds.map((f) => (
                  <div key={f.id} className="flex-between" style={{ fontSize: '0.85rem' }}>
                    <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
                      <span style={{ fontWeight: 600 }}>
                        {PLATFORM_NAMES[f.platform] ?? f.platform}
                      </span>
                      <span>{f.account_handle}</span>
                      <span className="text-muted">
                        · {FEED_CONTENT_TYPES[f.platform]?.find(c => c.value === f.content_type)?.label ?? f.content_type}
                      </span>
                      {f.last_checked_at && (
                        <span className="text-muted" style={{ fontSize: '0.75rem' }}>
                          · checked {new Date(f.last_checked_at).toLocaleString()}
                        </span>
                      )}
                      {f.last_error && (
                        <span style={{ color: 'var(--danger)', fontSize: '0.75rem' }}>
                          · {f.last_error}
                        </span>
                      )}
                    </div>
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={() => {
                        if (confirm(`Remove this ${f.platform} feed? Posts already discovered will remain.`)) {
                          deleteFeed.mutate(f.id);
                        }
                      }}
                      title="Remove feed"
                    >
                      <X size={14} />
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}

          {campaign.posts && campaign.posts.length > 0 ? (
            <div className="table-wrapper">
              <table>
                <thead>
                  <tr>
                    <th style={{ width: 28 }} />
                    <th>Platform</th>
                    <th>Title / Preview</th>
                    <SortHeader label="Community" sortKey="community" />
                    <SortHeader label="Posted" sortKey="posted_at" />
                    <SortHeader label="Likes" sortKey="likes" align="right" />
                    <SortHeader label="Comments" sortKey="comments" align="right" />
                    <SortHeader label="Views" sortKey="views" align="right" />
                  </tr>
                </thead>
                <tbody>
                  {sortedPosts.map((post) => (
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

          {/* Add Feed Modal */}
          {showAddFeed && (
            <div className="modal-overlay" onClick={() => setShowAddFeed(false)}>
              <div className="modal" onClick={(e) => e.stopPropagation()}>
                <div className="flex-between" style={{ marginBottom: 16 }}>
                  <h3 style={{ margin: 0 }}>Add Auto-Feed</h3>
                  <button
                    className="btn btn-ghost btn-sm"
                    onClick={() => setShowAddFeed(false)}
                  >
                    <X size={16} />
                  </button>
                </div>
                <p className="text-muted" style={{ marginBottom: 16, fontSize: '0.85rem' }}>
                  New posts on the chosen account will be auto-added to this campaign on each metric tick.
                  Only posts created after the feed is set up will be pulled in.
                </p>
                {feedCapableAccounts.length === 0 ? (
                  <div style={{ padding: 16, background: 'var(--bg-tertiary)', borderRadius: 'var(--radius-sm)' }}>
                    <p style={{ marginTop: 0, fontSize: '0.9rem' }}>
                      No feed-capable accounts connected to this profile yet.
                    </p>
                    <Link to="/settings" className="btn btn-primary btn-sm">
                      Go to Settings → Connect an account
                    </Link>
                  </div>
                ) : (
                  <form onSubmit={handleAddFeed}>
                    <div className="form-group">
                      <label>Connected account *</label>
                      <select
                        className="form-select"
                        value={feedAccountId}
                        onChange={(e) => {
                          const newId = e.target.value;
                          setFeedAccountId(newId);
                          const platform = feedCapableAccounts.find(a => a.id === newId)?.platform;
                          if (platform) {
                            setFeedContentType(FEED_CONTENT_TYPES[platform]?.[0]?.value ?? '');
                          }
                        }}
                        required
                      >
                        <option value="">Select an account...</option>
                        {feedCapableAccounts.map((a) => (
                          <option key={a.id} value={a.id}>
                            {PLATFORM_NAMES[a.platform] ?? a.platform} · {a.account_handle}
                          </option>
                        ))}
                      </select>
                    </div>
                    {selectedAccount && (
                      <div className="form-group">
                        <label>Content type *</label>
                        <select
                          className="form-select"
                          value={feedContentType}
                          onChange={(e) => setFeedContentType(e.target.value)}
                          required
                        >
                          {(FEED_CONTENT_TYPES[selectedAccount.platform] ?? []).map((c) => (
                            <option key={c.value} value={c.value}>{c.label}</option>
                          ))}
                        </select>
                      </div>
                    )}
                    {createFeed.isError && (
                      <p style={{ color: 'var(--danger)', fontSize: '0.825rem', marginBottom: 12 }}>
                        {(createFeed.error as Error)?.message ?? 'Failed to add feed'}
                      </p>
                    )}
                    <div style={{ display: 'flex', gap: 10 }}>
                      <button
                        type="submit"
                        className="btn btn-primary"
                        disabled={createFeed.isPending || !feedAccountId}
                      >
                        {createFeed.isPending ? 'Verifying...' : 'Add Feed'}
                      </button>
                      <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={() => setShowAddFeed(false)}
                      >
                        Cancel
                      </button>
                    </div>
                  </form>
                )}
              </div>
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
              Archiving hides the campaign from the dashboard and stops
              metric collection. Deleting permanently removes it and all
              its posts, metrics, and analyses.
            </p>
            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap', alignItems: 'center' }}>
              <button className="btn btn-danger" onClick={handleArchive}>
                Archive Campaign
              </button>
              {confirmPermanentDelete ? (
                <>
                  <button
                    className="btn btn-danger"
                    style={{ background: '#991b1b' }}
                    onClick={handlePermanentDelete}
                    disabled={deleteCampaign.isPending}
                  >
                    {deleteCampaign.isPending ? 'Deleting...' : 'Confirm Delete Forever'}
                  </button>
                  <button
                    className="btn btn-ghost btn-sm"
                    onClick={() => setConfirmPermanentDelete(false)}
                  >
                    Cancel
                  </button>
                </>
              ) : (
                <button
                  className="btn btn-ghost"
                  style={{ color: 'var(--danger)' }}
                  onClick={() => setConfirmPermanentDelete(true)}
                >
                  Delete Permanently
                </button>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
