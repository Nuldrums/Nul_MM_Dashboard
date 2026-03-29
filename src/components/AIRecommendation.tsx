import { Lightbulb } from 'lucide-react';

interface AIRecommendationProps {
  recommendation: {
    action: string;
    priority: string;
    reasoning: string;
    estimated_impact?: string;
  };
}

export default function AIRecommendation({
  recommendation,
}: AIRecommendationProps) {
  const priorityClass =
    recommendation.priority === 'high'
      ? 'badge-high'
      : recommendation.priority === 'medium'
        ? 'badge-medium'
        : 'badge-low';

  return (
    <div
      className="card"
      style={{ display: 'flex', gap: 14, alignItems: 'flex-start' }}
    >
      <div
        style={{
          width: 36,
          height: 36,
          borderRadius: 'var(--radius-sm)',
          background: 'var(--creamsicle-bg)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          flexShrink: 0,
        }}
      >
        <Lightbulb size={18} style={{ color: 'var(--creamsicle)' }} />
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div className="flex-between" style={{ marginBottom: 6 }}>
          <span style={{ fontWeight: 600, fontSize: '0.9rem' }}>
            {recommendation.action}
          </span>
          <span className={`badge ${priorityClass}`}>
            {recommendation.priority}
          </span>
        </div>
        <p
          style={{
            fontSize: '0.825rem',
            color: 'var(--text-secondary)',
            margin: 0,
            lineHeight: 1.5,
          }}
        >
          {recommendation.reasoning}
        </p>
        {recommendation.estimated_impact && (
          <span
            className="text-muted"
            style={{ display: 'block', marginTop: 6 }}
          >
            Estimated impact: {recommendation.estimated_impact}
          </span>
        )}
      </div>
    </div>
  );
}
