import { useState } from 'react';
import { ChevronDown, ChevronRight, ExternalLink } from 'lucide-react';
import type { Post, MetricSnapshot } from '../lib/types';
import { usePostMetrics } from '../hooks/useMetrics';
import PlatformBadge from './PlatformBadge';
import MetricSparkline from './MetricSparkline';

interface PostRowProps {
  post: Post;
}

export default function PostRow({ post }: PostRowProps) {
  const [expanded, setExpanded] = useState(false);
  const { data: metrics } = usePostMetrics(expanded ? post.id : '');

  const sparklineData =
    metrics?.map((m: MetricSnapshot) => ({
      date: m.snapshot_date,
      value: m.views + m.likes * 10 + m.comments * 20,
    })) ?? [];

  const latestMetric = metrics?.[metrics.length - 1];

  return (
    <>
      <tr
        style={{ cursor: 'pointer' }}
        onClick={() => setExpanded(!expanded)}
      >
        <td style={{ width: 28 }}>
          {expanded ? (
            <ChevronDown size={14} />
          ) : (
            <ChevronRight size={14} />
          )}
        </td>
        <td>
          <PlatformBadge platform={post.platform} />
        </td>
        <td>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <span>
              {post.title || post.body_preview || post.url || 'Untitled'}
            </span>
            {post.url && (
              <a
                href={post.url}
                target="_blank"
                rel="noopener noreferrer"
                onClick={(e) => e.stopPropagation()}
                style={{ color: 'var(--accent-primary)', display: 'flex' }}
              >
                <ExternalLink size={12} />
              </a>
            )}
          </div>
        </td>
        <td>{post.target_community ?? '--'}</td>
        <td className="text-muted">
          {post.posted_at
            ? new Date(post.posted_at).toLocaleDateString()
            : 'Not posted'}
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
          <td
            colSpan={6}
            style={{ padding: '12px 16px', background: 'var(--bg-secondary)' }}
          >
            <div
              style={{
                display: 'flex',
                gap: 24,
                alignItems: 'center',
                flexWrap: 'wrap',
              }}
            >
              <MetricSparkline
                data={sparklineData}
                width={140}
                height={40}
              />
              {latestMetric ? (
                <div
                  style={{ display: 'flex', gap: 16, fontSize: '0.8rem' }}
                >
                  <span>
                    Views:{' '}
                    <strong>
                      {latestMetric.views.toLocaleString()}
                    </strong>
                  </span>
                  <span>
                    Likes:{' '}
                    <strong>
                      {latestMetric.likes.toLocaleString()}
                    </strong>
                  </span>
                  <span>
                    Comments:{' '}
                    <strong>
                      {latestMetric.comments.toLocaleString()}
                    </strong>
                  </span>
                  <span>
                    Shares:{' '}
                    <strong>
                      {latestMetric.shares.toLocaleString()}
                    </strong>
                  </span>
                  <span>
                    Saves:{' '}
                    <strong>
                      {latestMetric.saves.toLocaleString()}
                    </strong>
                  </span>
                </div>
              ) : (
                <span className="text-muted">
                  No metrics collected yet
                </span>
              )}
            </div>
          </td>
        </tr>
      )}
    </>
  );
}
