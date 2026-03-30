import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { TrendingUp, Trash2 } from 'lucide-react';
import type { Campaign } from '../lib/types';
import { PLATFORM_COLORS } from '../lib/constants';
import { useDeleteCampaign } from '../hooks/useCampaigns';
import MetricSparkline from './MetricSparkline';

interface CampaignCardProps {
  campaign: Campaign;
}

function getScoreClass(score?: number) {
  if (score == null) return '';
  if (score >= 7) return 'score-high';
  if (score >= 4) return 'score-mid';
  return 'score-low';
}

export default function CampaignCard({ campaign }: CampaignCardProps) {
  const navigate = useNavigate();
  const deleteCampaign = useDeleteCampaign();
  const [confirmDelete, setConfirmDelete] = useState(false);

  const platforms = Array.from(
    new Set(campaign.posts?.map((p) => p.platform) ?? [])
  );

  const sparklineData =
    campaign.posts
      ?.filter((p) => p.posted_at)
      .sort((a, b) => (a.posted_at! > b.posted_at! ? 1 : -1))
      .map((_, i) => ({
        date: String(i),
        value:
          (campaign.metrics_summary?.avg_engagement ?? 1) * (i + 1) * 0.5 +
          i * 2,
      })) ?? [];

  const score = campaign.metrics_summary?.ai_score;

  return (
    <div
      className="card"
      style={{ cursor: 'pointer', transition: 'box-shadow 0.15s' }}
      onClick={() => navigate(`/campaigns/${campaign.id}`)}
      onMouseEnter={(e) =>
        (e.currentTarget.style.boxShadow = 'var(--shadow-md)')
      }
      onMouseLeave={(e) =>
        (e.currentTarget.style.boxShadow = 'var(--shadow-sm)')
      }
    >
      <div className="flex-between" style={{ marginBottom: 12 }}>
        <div>
          <h3 style={{ fontSize: '1rem', fontWeight: 600, margin: 0 }}>
            {campaign.name}
          </h3>
          <span className="text-muted">
            {campaign.product?.name ?? 'No product'}
          </span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span className={`badge badge-${campaign.status}`}>
            {campaign.status}
          </span>
          {confirmDelete ? (
            <span style={{ display: 'flex', gap: 4, alignItems: 'center' }} onClick={e => e.stopPropagation()}>
              <button
                className="btn btn-sm"
                style={{ fontSize: '0.7rem', padding: '2px 8px', background: 'var(--color-danger, #ef4444)', color: '#fff', border: 'none', borderRadius: 4, cursor: 'pointer' }}
                onClick={() => deleteCampaign.mutate({ id: campaign.id, permanent: true })}
              >
                Delete
              </button>
              <button
                className="btn btn-sm"
                style={{ fontSize: '0.7rem', padding: '2px 8px', background: 'transparent', color: 'var(--text-muted)', border: '1px solid var(--border)', borderRadius: 4, cursor: 'pointer' }}
                onClick={() => setConfirmDelete(false)}
              >
                Cancel
              </button>
            </span>
          ) : (
            <button
              style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', padding: 4, borderRadius: 4, display: 'flex', alignItems: 'center' }}
              title="Delete campaign"
              onClick={(e) => { e.stopPropagation(); setConfirmDelete(true); }}
              onMouseEnter={e => (e.currentTarget.style.color = 'var(--color-danger, #ef4444)')}
              onMouseLeave={e => (e.currentTarget.style.color = 'var(--text-muted)')}
            >
              <Trash2 size={14} />
            </button>
          )}
        </div>
      </div>

      <div className="flex-between" style={{ marginBottom: 8 }}>
        <div className="platform-icons">
          {platforms.map((p) => (
            <span
              key={p}
              className="platform-dot"
              style={{
                backgroundColor:
                  PLATFORM_COLORS[p] ?? PLATFORM_COLORS.other,
              }}
              title={p}
            />
          ))}
          {platforms.length === 0 && (
            <span className="text-muted">No posts yet</span>
          )}
        </div>
        <MetricSparkline data={sparklineData} />
      </div>

      <div className="flex-between">
        <span className="text-muted">
          {campaign.posts?.length ?? 0} post
          {(campaign.posts?.length ?? 0) !== 1 ? 's' : ''}
        </span>
        {score != null && score > 0 ? (
          <span
            className={`flex-gap ${getScoreClass(score)}`}
            style={{ fontWeight: 700, fontSize: '0.9rem' }}
          >
            <TrendingUp size={14} />
            {score.toFixed(1)}
          </span>
        ) : (
          <span className="text-muted">--</span>
        )}
      </div>
    </div>
  );
}
