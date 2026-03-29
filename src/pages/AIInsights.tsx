import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  Brain,
  Search,
  Database,
  Zap,
  BookOpen,
} from 'lucide-react';
import { apiFetch } from '../hooks/useApi';
import { useActiveProfile } from '../hooks/useActiveProfile';
import type { AIAnalysis } from '../lib/types';
import AIRecommendation from '../components/AIRecommendation';

interface KBStats {
  total_documents: number;
  campaigns_covered: number;
}

interface KBResult {
  id: string;
  content: string;
  metadata?: Record<string, any>;
  relevance?: number;
}

export default function AIInsights() {
  const { activeProfileId } = useActiveProfile();
  const profileParam = activeProfileId ? `?profile_id=${activeProfileId}` : '';
  const [searchQuery, setSearchQuery] = useState('');
  const [searchTerm, setSearchTerm] = useState('');

  const { data: latestAnalyses } = useQuery<AIAnalysis[]>({
    queryKey: ['ai', 'latest', activeProfileId ?? 'all'],
    queryFn: () => apiFetch<AIAnalysis[]>(`/ai/latest${profileParam}`),
  });

  const { data: kbStats } = useQuery<KBStats>({
    queryKey: ['ai', 'kb-stats'],
    queryFn: () => apiFetch<KBStats>('/ai/knowledge-base/stats'),
  });

  const { data: kbResults, isFetching: isSearching } = useQuery<KBResult[]>({
    queryKey: ['ai', 'kb-search', searchTerm],
    queryFn: () =>
      apiFetch<KBResult[]>(
        `/ai/knowledge-base/query?q=${encodeURIComponent(searchTerm)}`
      ),
    enabled: !!searchTerm,
  });

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    if (searchQuery.trim()) {
      setSearchTerm(searchQuery.trim());
    }
  };

  const handleRunAnalysis = async () => {
    try {
      await apiFetch('/ai/analyze', { method: 'POST' });
    } catch {
      // silently handle
    }
  };

  // Collect all patterns and recommendations across analyses
  const allPatterns =
    latestAnalyses?.flatMap((a) => a.patterns ?? []) ?? [];
  const allRecommendations =
    latestAnalyses?.flatMap((a) => a.recommendations ?? []) ?? [];

  // Cross-campaign insights: analyses without campaign_id
  const crossCampaign =
    latestAnalyses?.filter((a) => !a.campaign_id) ?? [];
  const perCampaign =
    latestAnalyses?.filter((a) => a.campaign_id) ?? [];

  const hasData =
    (latestAnalyses && latestAnalyses.length > 0) ||
    (kbStats && kbStats.total_documents > 0);

  if (!hasData) {
    return (
      <div>
        <div className="page-header">
          <h2>AI Insights</h2>
          <p>Your AI-powered marketing brain</p>
        </div>
        <div className="empty-state">
          <Brain />
          <h3>No AI analyses yet</h3>
          <p>
            Run your first analysis to get AI-powered insights about your
            marketing campaigns. The AI will analyze your posts, metrics,
            and patterns to provide actionable recommendations.
          </p>
          <button
            className="btn btn-primary"
            onClick={handleRunAnalysis}
          >
            <Zap size={16} /> Run First Analysis
          </button>
        </div>
      </div>
    );
  }

  return (
    <div>
      <div className="page-header">
        <h2>AI Insights</h2>
        <p>Your AI-powered marketing brain</p>
      </div>

      {/* Section 1: Latest Intelligence */}
      <div className="section">
        <h3 className="section-title">
          <Brain
            size={18}
            style={{
              display: 'inline',
              verticalAlign: 'middle',
              marginRight: 8,
            }}
          />
          Latest Intelligence
        </h3>

        {/* Per-Campaign Summaries */}
        {perCampaign.length > 0 && (
          <div style={{ marginBottom: 20 }}>
            <h4
              style={{
                fontSize: '0.85rem',
                fontWeight: 600,
                color: 'var(--text-secondary)',
                marginBottom: 12,
                textTransform: 'uppercase',
                letterSpacing: '0.04em',
              }}
            >
              Campaign Analyses
            </h4>
            <div
              style={{
                display: 'grid',
                gridTemplateColumns:
                  'repeat(auto-fill, minmax(340px, 1fr))',
                gap: 12,
              }}
            >
              {perCampaign.map((analysis) => (
                <div key={analysis.id} className="card">
                  <div
                    className="flex-between"
                    style={{ marginBottom: 8 }}
                  >
                    <span
                      style={{
                        fontSize: '0.8rem',
                        fontWeight: 600,
                        color: 'var(--accent-primary)',
                      }}
                    >
                      {analysis.analysis_type.replace(/_/g, ' ')}
                    </span>
                    <span className="text-muted">
                      {new Date(
                        analysis.analyzed_at
                      ).toLocaleDateString()}
                    </span>
                  </div>
                  <p
                    style={{
                      fontSize: '0.85rem',
                      lineHeight: 1.55,
                      margin: 0,
                    }}
                  >
                    {analysis.summary.length > 200
                      ? analysis.summary.slice(0, 200) + '...'
                      : analysis.summary}
                  </p>
                  {analysis.model_used && (
                    <span
                      className="text-muted"
                      style={{ display: 'block', marginTop: 8 }}
                    >
                      Model: {analysis.model_used}
                      {analysis.tokens_used &&
                        ` | ${analysis.tokens_used.toLocaleString()} tokens`}
                    </span>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Cross-Campaign Insights */}
        {crossCampaign.length > 0 && (
          <div style={{ marginBottom: 20 }}>
            <h4
              style={{
                fontSize: '0.85rem',
                fontWeight: 600,
                color: 'var(--text-secondary)',
                marginBottom: 12,
                textTransform: 'uppercase',
                letterSpacing: '0.04em',
              }}
            >
              Cross-Campaign Insights
            </h4>
            {crossCampaign.map((analysis) => (
              <div
                key={analysis.id}
                className="ai-callout"
                style={{ marginTop: 0, marginBottom: 12 }}
              >
                <Brain size={18} />
                <div className="ai-callout-text">
                  <div className="ai-callout-label">
                    {analysis.analysis_type.replace(/_/g, ' ')}
                  </div>
                  <div>{analysis.summary}</div>
                </div>
              </div>
            ))}
          </div>
        )}

        {/* Pattern Library */}
        {allPatterns.length > 0 && (
          <div>
            <h4
              style={{
                fontSize: '0.85rem',
                fontWeight: 600,
                color: 'var(--text-secondary)',
                marginBottom: 12,
                textTransform: 'uppercase',
                letterSpacing: '0.04em',
              }}
            >
              Pattern Library
            </h4>
            <div
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: 8,
              }}
            >
              {allPatterns.map((pat, i) => (
                <div key={i} className="card">
                  <div
                    className="flex-between"
                    style={{ marginBottom: 4 }}
                  >
                    <span
                      style={{
                        fontWeight: 600,
                        fontSize: '0.875rem',
                      }}
                    >
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
                        margin: '4px 0 0',
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

        {/* Recommendations */}
        {allRecommendations.length > 0 && (
          <div style={{ marginTop: 20 }}>
            <h4
              style={{
                fontSize: '0.85rem',
                fontWeight: 600,
                color: 'var(--text-secondary)',
                marginBottom: 12,
                textTransform: 'uppercase',
                letterSpacing: '0.04em',
              }}
            >
              Recommendations
            </h4>
            <div
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: 12,
              }}
            >
              {allRecommendations.map((rec, i) => (
                <AIRecommendation key={i} recommendation={rec} />
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Section 2: Knowledge Base Explorer */}
      <div className="section">
        <h3 className="section-title">
          <BookOpen
            size={18}
            style={{
              display: 'inline',
              verticalAlign: 'middle',
              marginRight: 8,
            }}
          />
          Knowledge Base Explorer
        </h3>

        {kbStats && (
          <div
            className="flex-gap mb-16"
            style={{ gap: 20 }}
          >
            <span className="flex-gap text-muted">
              <Database size={14} />
              {kbStats.total_documents} documents
            </span>
            <span className="text-muted">
              {kbStats.campaigns_covered} campaigns covered
            </span>
          </div>
        )}

        <form
          onSubmit={handleSearch}
          style={{ display: 'flex', gap: 8, marginBottom: 20 }}
        >
          <input
            className="form-input"
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search the knowledge base... (e.g., 'best time to post on Reddit')"
            style={{ flex: 1 }}
          />
          <button
            type="submit"
            className="btn btn-primary"
            disabled={isSearching || !searchQuery.trim()}
          >
            <Search size={16} />
            {isSearching ? 'Searching...' : 'Search'}
          </button>
        </form>

        {kbResults && kbResults.length > 0 && (
          <div className="kb-results">
            {kbResults.map((result) => (
              <div key={result.id} className="kb-card">
                <div
                  className="flex-between"
                  style={{ marginBottom: 8 }}
                >
                  <h4>{result.metadata?.title ?? 'Knowledge Entry'}</h4>
                  {result.relevance != null && (
                    <span className="text-muted">
                      {(result.relevance * 100).toFixed(0)}% match
                    </span>
                  )}
                </div>
                <p>{result.content}</p>
                {result.metadata?.campaign_id && (
                  <span className="text-muted" style={{ marginTop: 6, display: 'block' }}>
                    Campaign: {result.metadata.campaign_id}
                  </span>
                )}
              </div>
            ))}
          </div>
        )}

        {searchTerm && kbResults && kbResults.length === 0 && !isSearching && (
          <div className="empty-state" style={{ padding: '24px 0' }}>
            <Search />
            <p>No results found for "{searchTerm}"</p>
          </div>
        )}
      </div>
    </div>
  );
}
