import { useNavigate } from 'react-router-dom';
import { TrendingUp } from 'lucide-react';
import type { Campaign } from '../lib/types';
import { PLATFORM_COLORS } from '../lib/constants';
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

  const platforms = campaign.platforms?.length
    ? campaign.platforms
    : Array.from(new Set(campaign.posts?.map((p) => p.platform) ?? []));

  const postCount = campaign.post_count ?? campaign.posts?.length ?? 0;
  const totalEngagement =
    (campaign.total_likes ?? 0) +
    (campaign.total_comments ?? 0);
  const avgEngagement =
    campaign.metrics_summary?.avg_engagement ??
    (postCount > 0 ? totalEngagement / postCount : 0);

  const sparklineData =
    campaign.posts
      ?.filter((p) => p.posted_at)
      .sort((a, b) => (a.posted_at! > b.posted_at! ? 1 : -1))
      .map((_, i) => ({
        date: String(i),
        value: (avgEngagement || 1) * (i + 1) * 0.5 + i * 2,
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
        <span className={`badge badge-${campaign.status}`}>
          {campaign.status}
        </span>
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
          {postCount} post{postCount !== 1 ? 's' : ''}
          {totalEngagement > 0 && ` · ${totalEngagement.toLocaleString()} engagement`}
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
