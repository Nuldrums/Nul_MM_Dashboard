import { useState } from 'react';
import { ChevronDown, ChevronRight, ExternalLink, Trash2, Plus, Pencil, X, Check } from 'lucide-react';
import { open } from '@tauri-apps/plugin-shell';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import type { Post, MetricSnapshot } from '../lib/types';
import { usePostMetrics } from '../hooks/useMetrics';
import { apiFetch } from '../hooks/useApi';
import PlatformBadge from './PlatformBadge';
import MetricSparkline from './MetricSparkline';

interface PostRowProps {
  post: Post;
}

interface MetricFormData {
  snapshot_date: string;
  views: string;
  likes: string;
  comments: string;
  shares: string;
  saves: string;
}

interface PostFormData {
  platform: string;
  post_type: string;
  title: string;
  url: string;
  target_community: string;
  posted_at: string;
  body_preview: string;
}

const emptyMetricForm = (): MetricFormData => ({
  snapshot_date: new Date().toISOString().slice(0, 10),
  views: '',
  likes: '',
  comments: '',
  shares: '',
  saves: '',
});

const PLATFORMS = ['reddit', 'x', 'youtube', 'discord', 'tiktok', 'instagram', 'linkedin', 'other'];
const POST_TYPES = ['text', 'image', 'video_short', 'video_long', 'thread', 'comment', 'link', 'self_promo'];

const smallBtn: React.CSSProperties = {
  background: 'none',
  border: 'none',
  cursor: 'pointer',
  padding: 2,
  display: 'flex',
  alignItems: 'center',
  borderRadius: 3,
};

const inputStyle: React.CSSProperties = { padding: '4px 8px', fontSize: '0.8rem' };
const labelStyle: React.CSSProperties = { fontSize: '0.7rem', color: 'var(--text-muted)' };
const fieldCol: React.CSSProperties = { display: 'flex', flexDirection: 'column', gap: 2 };

export default function PostRow({ post }: PostRowProps) {
  const [expanded, setExpanded] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [showMetricForm, setShowMetricForm] = useState(false);
  const [metricForm, setMetricForm] = useState<MetricFormData>(emptyMetricForm);
  const [editingPost, setEditingPost] = useState(false);
  const [postForm, setPostForm] = useState<PostFormData | null>(null);
  const [editingMetricId, setEditingMetricId] = useState<number | null>(null);
  const [editMetricForm, setEditMetricForm] = useState<MetricFormData | null>(null);
  const [confirmDeleteMetricId, setConfirmDeleteMetricId] = useState<number | null>(null);
  const queryClient = useQueryClient();
  const { data: metrics } = usePostMetrics(expanded ? post.id : '');

  const invalidateAll = () => {
    queryClient.invalidateQueries({ queryKey: ['metrics', 'post', post.id] });
    queryClient.invalidateQueries({ queryKey: ['metrics', 'campaign'] });
    queryClient.invalidateQueries({ queryKey: ['campaigns'] });
  };

  const deletePost = useMutation({
    mutationFn: () => apiFetch(`/posts/${post.id}`, { method: 'DELETE' }),
    onSuccess: invalidateAll,
    onError: invalidateAll,
  });

  const updatePost = useMutation({
    mutationFn: (data: PostFormData) =>
      apiFetch(`/posts/${post.id}`, {
        method: 'PUT',
        body: JSON.stringify({
          platform: data.platform,
          post_type: data.post_type,
          title: data.title || null,
          url: data.url || null,
          target_community: data.target_community || null,
          posted_at: data.posted_at || null,
          body_preview: data.body_preview || null,
        }),
      }),
    onSuccess: () => {
      invalidateAll();
      setEditingPost(false);
      setPostForm(null);
    },
  });

  const addMetric = useMutation({
    mutationFn: (data: MetricFormData) =>
      apiFetch(`/posts/${post.id}/metrics`, {
        method: 'POST',
        body: JSON.stringify({
          snapshot_date: data.snapshot_date,
          views: parseInt(data.views) || 0,
          likes: parseInt(data.likes) || 0,
          comments: parseInt(data.comments) || 0,
          shares: parseInt(data.shares) || 0,
          saves: parseInt(data.saves) || 0,
        }),
      }),
    onSuccess: () => {
      invalidateAll();
      setShowMetricForm(false);
      setMetricForm(emptyMetricForm());
    },
  });

  const updateMetric = useMutation({
    mutationFn: ({ id, data }: { id: number; data: MetricFormData }) =>
      apiFetch(`/metrics/${id}`, {
        method: 'PUT',
        body: JSON.stringify({
          snapshot_date: data.snapshot_date,
          views: parseInt(data.views) || 0,
          likes: parseInt(data.likes) || 0,
          comments: parseInt(data.comments) || 0,
          shares: parseInt(data.shares) || 0,
          saves: parseInt(data.saves) || 0,
        }),
      }),
    onSuccess: () => {
      invalidateAll();
      setEditingMetricId(null);
      setEditMetricForm(null);
    },
  });

  const deleteMetric = useMutation({
    mutationFn: (id: number) => apiFetch(`/metrics/${id}`, { method: 'DELETE' }),
    onSuccess: () => {
      invalidateAll();
      setConfirmDeleteMetricId(null);
    },
  });

  const sparklineData =
    metrics?.map((m: MetricSnapshot) => ({
      date: m.snapshot_date,
      value: m.views + m.likes * 10 + m.comments * 20,
    })) ?? [];

  const latestMetric = metrics?.[metrics.length - 1];

  const startEditPost = (e?: React.MouseEvent) => {
    e?.stopPropagation();
    setPostForm({
      platform: post.platform,
      post_type: post.post_type,
      title: post.title || '',
      url: post.url || '',
      target_community: post.target_community || '',
      posted_at: post.posted_at ? post.posted_at.slice(0, 10) : '',
      body_preview: post.body_preview || '',
    });
    setEditingPost(true);
    if (!expanded) setExpanded(true);
  };

  const startEditMetric = (m: MetricSnapshot) => {
    setEditMetricForm({
      snapshot_date: m.snapshot_date,
      views: String(m.views),
      likes: String(m.likes),
      comments: String(m.comments),
      shares: String(m.shares),
      saves: String(m.saves),
    });
    setEditingMetricId(m.id);
  };

  return (
    <>
      <tr
        style={{ cursor: 'pointer' }}
        onClick={() => setExpanded(!expanded)}
      >
        <td style={{ width: 28 }}>
          {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </td>
        <td><PlatformBadge platform={post.platform} /></td>
        <td>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <span>{post.title || post.body_preview || post.url || 'Untitled'}</span>
            {post.url && (
              <button
                onClick={(e) => { e.stopPropagation(); open(post.url!); }}
                style={{ color: 'var(--accent-primary)', display: 'flex', background: 'none', border: 'none', cursor: 'pointer', padding: 0 }}
              >
                <ExternalLink size={12} />
              </button>
            )}
            <button
              onClick={startEditPost}
              style={{ ...smallBtn, color: 'var(--text-muted)', opacity: 0.5 }}
              title="Edit post"
              onMouseEnter={e => { e.currentTarget.style.opacity = '1'; e.currentTarget.style.color = 'var(--accent-primary)'; }}
              onMouseLeave={e => { e.currentTarget.style.opacity = '0.5'; e.currentTarget.style.color = 'var(--text-muted)'; }}
            >
              <Pencil size={11} />
            </button>
          </div>
        </td>
        <td>{post.target_community ?? '--'}</td>
        <td className="text-muted">
          {post.posted_at ? new Date(post.posted_at).toLocaleDateString() : 'Not posted'}
        </td>
        <td style={{ textAlign: 'right' }}>
          {latestMetric ? (
            <span>{latestMetric.views.toLocaleString()} views</span>
          ) : (
            <span className="text-muted">--</span>
          )}
        </td>
      </tr>
      {expanded && (
        <tr>
          <td colSpan={6} style={{ padding: '12px 16px', background: 'var(--bg-secondary)' }}>
            {/* Post edit form */}
            {editingPost && postForm ? (
              <form
                onSubmit={(e) => { e.preventDefault(); updatePost.mutate(postForm); }}
                style={{ marginBottom: 12, padding: 12, background: 'var(--surface-card, var(--bg-primary))', borderRadius: 'var(--radius-sm, 6px)', border: '1px solid var(--border-medium)' }}
              >
                <div style={{ fontSize: '0.75rem', fontWeight: 600, marginBottom: 8, color: 'var(--text-muted)' }}>EDIT POST</div>
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))', gap: 10 }}>
                  <div style={fieldCol}>
                    <label style={labelStyle}>Platform</label>
                    <select className="form-input" value={postForm.platform} onChange={e => setPostForm(f => ({ ...f!, platform: e.target.value }))} style={inputStyle}>
                      {PLATFORMS.map(p => <option key={p} value={p}>{p}</option>)}
                    </select>
                  </div>
                  <div style={fieldCol}>
                    <label style={labelStyle}>Type</label>
                    <select className="form-input" value={postForm.post_type} onChange={e => setPostForm(f => ({ ...f!, post_type: e.target.value }))} style={inputStyle}>
                      {POST_TYPES.map(t => <option key={t} value={t}>{t}</option>)}
                    </select>
                  </div>
                  <div style={{ ...fieldCol, gridColumn: 'span 2' }}>
                    <label style={labelStyle}>Title</label>
                    <input className="form-input" value={postForm.title} onChange={e => setPostForm(f => ({ ...f!, title: e.target.value }))} style={inputStyle} />
                  </div>
                  <div style={{ ...fieldCol, gridColumn: 'span 2' }}>
                    <label style={labelStyle}>URL</label>
                    <input className="form-input" value={postForm.url} onChange={e => setPostForm(f => ({ ...f!, url: e.target.value }))} style={inputStyle} />
                  </div>
                  <div style={fieldCol}>
                    <label style={labelStyle}>Community</label>
                    <input className="form-input" value={postForm.target_community} onChange={e => setPostForm(f => ({ ...f!, target_community: e.target.value }))} style={inputStyle} />
                  </div>
                  <div style={fieldCol}>
                    <label style={labelStyle}>Posted Date</label>
                    <input className="form-input" type="date" value={postForm.posted_at} onChange={e => setPostForm(f => ({ ...f!, posted_at: e.target.value }))} style={inputStyle} />
                  </div>
                  <div style={{ ...fieldCol, gridColumn: '1 / -1' }}>
                    <label style={labelStyle}>Body Preview</label>
                    <input className="form-input" value={postForm.body_preview} onChange={e => setPostForm(f => ({ ...f!, body_preview: e.target.value }))} style={inputStyle} />
                  </div>
                </div>
                <div style={{ display: 'flex', gap: 8, marginTop: 10, justifyContent: 'flex-end' }}>
                  <button type="button" className="btn btn-ghost btn-sm" style={{ fontSize: '0.75rem', padding: '5px 12px' }} onClick={() => { setEditingPost(false); setPostForm(null); }}>
                    Cancel
                  </button>
                  <button type="submit" className="btn btn-primary btn-sm" style={{ fontSize: '0.75rem', padding: '5px 12px' }} disabled={updatePost.isPending}>
                    {updatePost.isPending ? 'Saving...' : 'Save Changes'}
                  </button>
                </div>
              </form>
            ) : null}

            {/* Metrics summary + actions */}
            <div style={{ display: 'flex', gap: 16, alignItems: 'center', flexWrap: 'wrap' }}>
              <MetricSparkline data={sparklineData} width={140} height={40} />
              {latestMetric ? (
                <div style={{ display: 'flex', gap: 16, fontSize: '0.8rem' }}>
                  <span>Views: <strong>{latestMetric.views.toLocaleString()}</strong></span>
                  <span>Likes: <strong>{latestMetric.likes.toLocaleString()}</strong></span>
                  <span>Comments: <strong>{latestMetric.comments.toLocaleString()}</strong></span>
                  <span>Shares: <strong>{latestMetric.shares.toLocaleString()}</strong></span>
                  <span>Saves: <strong>{latestMetric.saves.toLocaleString()}</strong></span>
                </div>
              ) : (
                <span className="text-muted">No metrics collected yet</span>
              )}
            </div>

            {/* Action buttons — always on their own line for visibility */}
            <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginTop: 10 }}>
              {!showMetricForm && (
                <button
                  style={{ background: 'none', border: '1px solid var(--border-medium)', cursor: 'pointer', color: 'var(--accent-primary)', padding: '3px 10px', borderRadius: 4, display: 'flex', alignItems: 'center', gap: 4, fontSize: '0.75rem' }}
                  onClick={() => setShowMetricForm(true)}
                >
                  <Plus size={12} /> Add Metrics
                </button>
              )}
              {!editingPost && (
                <button
                  style={{ background: 'none', border: '1px solid var(--border-medium)', cursor: 'pointer', color: 'var(--accent-primary)', padding: '3px 10px', borderRadius: 4, display: 'flex', alignItems: 'center', gap: 4, fontSize: '0.75rem' }}
                  onClick={() => startEditPost()}
                >
                  <Pencil size={12} /> Edit Post
                </button>
              )}
              <div style={{ marginLeft: 'auto' }}>
                {confirmDelete ? (
                  <span style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                    <button
                      className="btn btn-sm"
                      style={{ fontSize: '0.75rem', padding: '3px 10px', background: 'var(--danger, #ef4444)', color: '#fff', border: 'none', borderRadius: 4, cursor: 'pointer' }}
                      onClick={() => deletePost.mutate()}
                      disabled={deletePost.isPending}
                    >
                      {deletePost.isPending ? 'Deleting...' : 'Confirm Delete'}
                    </button>
                    <button
                      className="btn btn-sm"
                      style={{ fontSize: '0.75rem', padding: '3px 10px', background: 'transparent', color: 'var(--text-muted)', border: '1px solid var(--border-medium)', borderRadius: 4, cursor: 'pointer' }}
                      onClick={() => setConfirmDelete(false)}
                    >
                      Cancel
                    </button>
                  </span>
                ) : (
                  <button
                    style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', padding: 4, borderRadius: 4, display: 'flex', alignItems: 'center', gap: 4, fontSize: '0.75rem' }}
                    onClick={() => setConfirmDelete(true)}
                    onMouseEnter={e => (e.currentTarget.style.color = 'var(--danger, #ef4444)')}
                    onMouseLeave={e => (e.currentTarget.style.color = 'var(--text-muted)')}
                  >
                    <Trash2 size={12} /> Delete
                  </button>
                )}
              </div>
            </div>

            {/* Manual metric entry form */}
            {showMetricForm && (
              <form
                onSubmit={(e) => { e.preventDefault(); addMetric.mutate(metricForm); }}
                style={{ marginTop: 12, padding: 12, background: 'var(--surface-card, var(--bg-primary))', borderRadius: 'var(--radius-sm, 6px)', border: '1px solid var(--border-medium)' }}
              >
                <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap', alignItems: 'end' }}>
                  <div style={fieldCol}>
                    <label style={labelStyle}>Date</label>
                    <input className="form-input" type="date" value={metricForm.snapshot_date} onChange={e => setMetricForm(f => ({ ...f, snapshot_date: e.target.value }))} style={{ ...inputStyle, width: 130 }} />
                  </div>
                  {(['views', 'likes', 'comments', 'shares', 'saves'] as const).map(field => (
                    <div key={field} style={fieldCol}>
                      <label style={{ ...labelStyle, textTransform: 'capitalize' }}>{field}</label>
                      <input className="form-input" type="number" min="0" placeholder="0" value={metricForm[field]} onChange={e => setMetricForm(f => ({ ...f, [field]: e.target.value }))} style={{ ...inputStyle, width: 80 }} />
                    </div>
                  ))}
                  <button type="submit" className="btn btn-primary btn-sm" style={{ fontSize: '0.75rem', padding: '5px 12px' }} disabled={addMetric.isPending}>
                    {addMetric.isPending ? 'Saving...' : 'Save'}
                  </button>
                  <button type="button" className="btn btn-ghost btn-sm" style={{ fontSize: '0.75rem', padding: '5px 12px' }} onClick={() => { setShowMetricForm(false); setMetricForm(emptyMetricForm()); }}>
                    Cancel
                  </button>
                </div>
              </form>
            )}

            {/* Metric snapshots list (editable) */}
            {metrics && metrics.length > 0 && (
              <div style={{ marginTop: 12 }}>
                <div style={{ fontSize: '0.7rem', color: 'var(--text-muted)', fontWeight: 600, marginBottom: 4 }}>METRIC HISTORY</div>
                <table style={{ width: '100%', fontSize: '0.78rem', borderCollapse: 'collapse' }}>
                  <thead>
                    <tr style={{ borderBottom: '1px solid var(--border-medium)' }}>
                      {['Date', 'Views', 'Likes', 'Comments', 'Shares', 'Saves', 'Source', ''].map((h, i) => (
                        <th key={h || i} style={{ textAlign: i === 0 || i === 6 ? 'left' : 'right', padding: '4px 8px', fontWeight: 500, color: 'var(--text-muted)', fontSize: '0.7rem', ...(i === 7 ? { width: 80 } : {}) }}>{h}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {metrics.map((m: MetricSnapshot) => (
                      editingMetricId === m.id && editMetricForm ? (
                        <tr key={m.id} style={{ background: 'var(--surface-card, var(--bg-primary))' }}>
                          <td style={{ padding: '4px 8px' }}>
                            <input className="form-input" type="date" value={editMetricForm.snapshot_date} onChange={e => setEditMetricForm(f => ({ ...f!, snapshot_date: e.target.value }))} style={{ ...inputStyle, width: 120 }} />
                          </td>
                          {(['views', 'likes', 'comments', 'shares', 'saves'] as const).map(field => (
                            <td key={field} style={{ padding: '4px 8px' }}>
                              <input className="form-input" type="number" min="0" value={editMetricForm[field]} onChange={e => setEditMetricForm(f => ({ ...f!, [field]: e.target.value }))} style={{ ...inputStyle, width: 70, textAlign: 'right' }} />
                            </td>
                          ))}
                          <td style={{ padding: '4px 8px', textAlign: 'left', color: 'var(--text-muted)', fontSize: '0.7rem' }}>{m.fetched_via}</td>
                          <td style={{ padding: '4px 8px', textAlign: 'right' }}>
                            <div style={{ display: 'flex', gap: 4, justifyContent: 'flex-end' }}>
                              <button style={{ ...smallBtn, color: 'var(--accent-primary)' }} title="Save" onClick={() => updateMetric.mutate({ id: m.id, data: editMetricForm })} disabled={updateMetric.isPending}>
                                <Check size={14} />
                              </button>
                              <button style={{ ...smallBtn, color: 'var(--text-muted)' }} title="Cancel" onClick={() => { setEditingMetricId(null); setEditMetricForm(null); }}>
                                <X size={14} />
                              </button>
                            </div>
                          </td>
                        </tr>
                      ) : (
                        <tr key={m.id} style={{ borderBottom: '1px solid var(--border-light, var(--border-medium))' }}>
                          <td style={{ padding: '4px 8px' }}>{m.snapshot_date}</td>
                          <td style={{ padding: '4px 8px', textAlign: 'right' }}>{m.views.toLocaleString()}</td>
                          <td style={{ padding: '4px 8px', textAlign: 'right' }}>{m.likes.toLocaleString()}</td>
                          <td style={{ padding: '4px 8px', textAlign: 'right' }}>{m.comments.toLocaleString()}</td>
                          <td style={{ padding: '4px 8px', textAlign: 'right' }}>{m.shares.toLocaleString()}</td>
                          <td style={{ padding: '4px 8px', textAlign: 'right' }}>{m.saves.toLocaleString()}</td>
                          <td style={{ padding: '4px 8px', textAlign: 'left', color: 'var(--text-muted)', fontSize: '0.7rem' }}>{m.fetched_via}</td>
                          <td style={{ padding: '4px 8px', textAlign: 'right' }}>
                            <div style={{ display: 'flex', gap: 4, justifyContent: 'flex-end' }}>
                              <button
                                style={{ ...smallBtn, color: 'var(--text-muted)' }}
                                title="Edit"
                                onClick={() => startEditMetric(m)}
                                onMouseEnter={e => (e.currentTarget.style.color = 'var(--accent-primary)')}
                                onMouseLeave={e => (e.currentTarget.style.color = 'var(--text-muted)')}
                              >
                                <Pencil size={12} />
                              </button>
                              {confirmDeleteMetricId === m.id ? (
                                <>
                                  <button style={{ ...smallBtn, color: 'var(--danger, #ef4444)' }} title="Confirm delete" onClick={() => deleteMetric.mutate(m.id)} disabled={deleteMetric.isPending}>
                                    <Check size={12} />
                                  </button>
                                  <button style={{ ...smallBtn, color: 'var(--text-muted)' }} title="Cancel" onClick={() => setConfirmDeleteMetricId(null)}>
                                    <X size={12} />
                                  </button>
                                </>
                              ) : (
                                <button
                                  style={{ ...smallBtn, color: 'var(--text-muted)' }}
                                  title="Delete"
                                  onClick={() => setConfirmDeleteMetricId(m.id)}
                                  onMouseEnter={e => (e.currentTarget.style.color = 'var(--danger, #ef4444)')}
                                  onMouseLeave={e => (e.currentTarget.style.color = 'var(--text-muted)')}
                                >
                                  <Trash2 size={12} />
                                </button>
                              )}
                            </div>
                          </td>
                        </tr>
                      )
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </td>
        </tr>
      )}
    </>
  );
}
