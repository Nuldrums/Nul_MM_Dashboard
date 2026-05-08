import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Plus, ArrowLeft, Package, Brain, RefreshCw } from 'lucide-react';
import { useCreateCampaign } from '../hooks/useCampaigns';
import { useActiveProfile } from '../hooks/useActiveProfile';
import { apiFetch } from '../hooks/useApi';
import TagInput from '../components/TagInput';
import type { Product } from '../lib/types';

const GOALS = [
  { value: 'drive_sales', label: 'Drive Sales' },
  { value: 'awareness', label: 'Brand Awareness' },
  { value: 'traffic', label: 'Drive Traffic' },
  { value: 'community_growth', label: 'Community Growth' },
];

export default function PostComposer() {
  const navigate = useNavigate();
  const createCampaign = useCreateCampaign();
  const queryClient = useQueryClient();
  const { activeProfileId } = useActiveProfile();
  const profileParam = activeProfileId ? `?profile_id=${activeProfileId}` : '';

  const { data: products } = useQuery<Product[]>({
    queryKey: ['products', activeProfileId ?? 'all'],
    queryFn: () => apiFetch<Product[]>(`/products${profileParam}`),
  });

  const [name, setName] = useState('');
  const [productId, setProductId] = useState('');
  const [goal, setGoal] = useState('awareness');
  const [audienceTags, setAudienceTags] = useState<string[]>([]);
  const [campaignTags, setCampaignTags] = useState<string[]>([]);
  const [startDate, setStartDate] = useState('');
  const [endDate, setEndDate] = useState('');
  const [notes, setNotes] = useState('');

  // Inline product creation
  const [showNewProduct, setShowNewProduct] = useState(false);
  const [newProductName, setNewProductName] = useState('');
  const [newProductType, setNewProductType] = useState('paid_software');
  const [newProductUrl, setNewProductUrl] = useState('');
  const [newProductPrice, setNewProductPrice] = useState('');
  const [creatingProduct, setCreatingProduct] = useState(false);
  const [error, setError] = useState('');

  // AI Recommendations
  const [aiLoading, setAiLoading] = useState(false);
  const [aiRecs, setAiRecs] = useState<any>(null);
  const [aiError, setAiError] = useState('');

  const handleCreateProduct = async () => {
    if (!newProductName.trim()) return;
    setCreatingProduct(true);
    try {
      const product = await apiFetch<Product>('/products', {
        method: 'POST',
        body: JSON.stringify({
          name: newProductName,
          type: newProductType,
          url: newProductUrl || undefined,
          price: newProductPrice ? parseFloat(newProductPrice) : undefined,
          profile_id: activeProfileId || undefined,
          tags: [],
        }),
      });
      setProductId(product.id);
      setShowNewProduct(false);
      setNewProductName('');
      setNewProductUrl('');
      setNewProductPrice('');
      queryClient.invalidateQueries({ queryKey: ['products'] });
    } catch (err: any) {
      console.error('[PostComposer] Product creation failed:', err);
      setError(err?.message || 'Failed to create product. Check if the backend is running.');
    } finally {
      setCreatingProduct(false);
    }
  };

  const selectedProduct = products?.find((p) => p.id === productId);

  const handleGetRecommendations = async () => {
    if (!selectedProduct) return;
    setAiLoading(true);
    setAiError('');
    setAiRecs(null);
    try {
      const result = await apiFetch<any>('/ai/campaign-recommendations', {
        method: 'POST',
        body: JSON.stringify({
          product_name: selectedProduct.name,
          product_type: selectedProduct.type,
          product_description: selectedProduct.description || '',
          goal: goal || undefined,
          target_audience: audienceTags.join(', ') || undefined,
          platforms: [],
        }),
      });
      if (result?.success) {
        setAiRecs(result.recommendations);
      } else {
        setAiError(result?.error || 'Failed to get recommendations');
      }
    } catch (e) {
      setAiError(String(e));
    } finally {
      setAiLoading(false);
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (!name.trim()) {
      setError('Campaign name is required.');
      return;
    }
    if (!productId) {
      setError('Please select or create a product first.');
      return;
    }

    createCampaign.mutate(
      {
        name,
        product_id: productId,
        profile_id: activeProfileId || undefined,
        goal,
        target_audience: audienceTags.length > 0 ? audienceTags : undefined,
        tags: campaignTags.length > 0 ? campaignTags : undefined,
        start_date: startDate || undefined,
        end_date: endDate || undefined,
        notes: notes || undefined,
        status: 'active',
      } as any,
      {
        onSuccess: (data) => {
          navigate(`/campaigns/${data.id}`);
        },
        onError: (err: any) => {
          setError(err?.message || 'Failed to create campaign. Please try again.');
        },
      }
    );
  };

  return (
    <div>
      <div className="page-header">
        <button
          className="btn btn-ghost"
          onClick={() => navigate('/')}
          style={{ marginBottom: 8 }}
        >
          <ArrowLeft size={16} /> Back
        </button>
        <h2>New Campaign</h2>
        <p>Set up a new marketing campaign to track</p>
      </div>

      <div className="card" style={{ maxWidth: 640 }}>
        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label>Campaign Name *</label>
            <input
              className="form-input"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g., Trik Klip Product Hunt Launch"
              required
            />
          </div>

          <div className="form-group">
            <label>Product</label>
            <div style={{ display: 'flex', gap: 8 }}>
              <select
                className="form-select"
                value={productId}
                onChange={(e) => setProductId(e.target.value)}
                style={{ flex: 1 }}
              >
                <option value="">-- Select product --</option>
                {products?.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => setShowNewProduct(!showNewProduct)}
              >
                <Package size={14} /> New
              </button>
            </div>
          </div>

          {showNewProduct && (
            <div
              style={{
                background: 'var(--bg-secondary)',
                borderRadius: 'var(--radius-sm)',
                padding: 16,
                marginBottom: 16,
              }}
            >
              <div className="form-group">
                <label>Product Name *</label>
                <input
                  className="form-input"
                  type="text"
                  value={newProductName}
                  onChange={(e) => setNewProductName(e.target.value)}
                  placeholder="e.g., Trik Klip"
                />
              </div>
              <div className="form-row">
                <div className="form-group">
                  <label>Type</label>
                  <select
                    className="form-select"
                    value={newProductType}
                    onChange={(e) => setNewProductType(e.target.value)}
                  >
                    <option value="paid_software">Paid Software</option>
                    <option value="free_tool">Free Tool</option>
                    <option value="interactive_page">Interactive Page</option>
                    <option value="content">Content</option>
                  </select>
                </div>
                <div className="form-group">
                  <label>Price</label>
                  <input
                    className="form-input"
                    type="number"
                    step="0.01"
                    value={newProductPrice}
                    onChange={(e) => setNewProductPrice(e.target.value)}
                    placeholder="0.00"
                  />
                </div>
              </div>
              <div className="form-group">
                <label>URL</label>
                <input
                  className="form-input"
                  type="url"
                  value={newProductUrl}
                  onChange={(e) => setNewProductUrl(e.target.value)}
                  placeholder="https://..."
                />
              </div>
              <button
                type="button"
                className="btn btn-primary btn-sm"
                onClick={handleCreateProduct}
                disabled={creatingProduct || !newProductName.trim()}
              >
                <Plus size={14} /> Create Product
              </button>
            </div>
          )}

          <div className="form-group">
            <label>Goal</label>
            <select
              className="form-select"
              value={goal}
              onChange={(e) => setGoal(e.target.value)}
            >
              {GOALS.map((g) => (
                <option key={g.value} value={g.value}>
                  {g.label}
                </option>
              ))}
            </select>
          </div>

          <div className="form-group">
            <label>Target Audience</label>
            <TagInput
              tags={audienceTags}
              onChange={setAudienceTags}
              placeholder="e.g., Indie developers, content creators"
            />
          </div>

          <div className="form-group">
            <label>Tags</label>
            <TagInput
              tags={campaignTags}
              onChange={setCampaignTags}
              placeholder="e.g., product-launch, Q2"
            />
          </div>

          <div className="form-row">
            <div className="form-group">
              <label>Start Date</label>
              <input
                className="form-input"
                type="date"
                value={startDate}
                onChange={(e) => setStartDate(e.target.value)}
              />
            </div>
            <div className="form-group">
              <label>End Date</label>
              <input
                className="form-input"
                type="date"
                value={endDate}
                onChange={(e) => setEndDate(e.target.value)}
              />
            </div>
          </div>

          <div className="form-group">
            <label>Notes</label>
            <textarea
              className="form-textarea"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              placeholder="Any additional notes about this campaign..."
            />
          </div>

          {/* AI Recommendations */}
          {selectedProduct && (
            <div style={{ marginBottom: 16 }}>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={handleGetRecommendations}
                disabled={aiLoading}
              >
                {aiLoading ? (
                  <><RefreshCw size={14} className="spin" /> Getting Recommendations...</>
                ) : (
                  <><Brain size={14} /> Get AI Strategy Recommendations</>
                )}
              </button>

              {aiError && (
                <div style={{
                  marginTop: 8, padding: '8px 12px', fontSize: '0.85rem',
                  background: 'color-mix(in srgb, var(--danger, #e53e3e) 15%, transparent)',
                  border: '1px solid var(--danger, #e53e3e)',
                  borderRadius: 'var(--radius-sm)',
                }}>{aiError}</div>
              )}

              {aiRecs && (
                <div style={{
                  marginTop: 12, padding: 16,
                  background: 'var(--bg-secondary)',
                  borderRadius: 'var(--radius-sm)',
                  border: '1px solid var(--border)',
                }}>
                  <h4 style={{ margin: '0 0 12px', display: 'flex', alignItems: 'center', gap: 6 }}>
                    <Brain size={16} /> AI Strategy Recommendations
                    <span className="text-muted" style={{ fontSize: '0.75rem', fontWeight: 400 }}>
                      Confidence: {aiRecs.confidence || 'unknown'}
                      {aiRecs.based_on_campaigns > 0 && ` (based on ${aiRecs.based_on_campaigns} past campaigns)`}
                    </span>
                  </h4>

                  {aiRecs.platform_recommendations?.length > 0 && (
                    <div style={{ marginBottom: 12 }}>
                      <h5 style={{ margin: '0 0 6px', fontSize: '0.85rem' }}>Platform Priorities</h5>
                      {aiRecs.platform_recommendations.map((r: any, i: number) => (
                        <div key={i} style={{ fontSize: '0.8rem', marginBottom: 4, paddingLeft: 8 }}>
                          <strong>{r.platform}</strong> ({r.priority}) — {r.reasoning}
                          {r.suggested_communities?.length > 0 && (
                            <div className="text-muted" style={{ paddingLeft: 8 }}>
                              Targets: {r.suggested_communities.join(', ')}
                            </div>
                          )}
                        </div>
                      ))}
                    </div>
                  )}

                  {aiRecs.content_strategy?.length > 0 && (
                    <div style={{ marginBottom: 12 }}>
                      <h5 style={{ margin: '0 0 6px', fontSize: '0.85rem' }}>Content Strategy</h5>
                      {aiRecs.content_strategy.map((r: any, i: number) => (
                        <div key={i} style={{ fontSize: '0.8rem', marginBottom: 4, paddingLeft: 8 }}>
                          <strong>{r.format}</strong> ({r.priority}) — {r.reasoning}
                        </div>
                      ))}
                    </div>
                  )}

                  {aiRecs.timing_suggestions?.length > 0 && (
                    <div style={{ marginBottom: 12 }}>
                      <h5 style={{ margin: '0 0 6px', fontSize: '0.85rem' }}>Timing</h5>
                      {aiRecs.timing_suggestions.map((r: any, i: number) => (
                        <div key={i} style={{ fontSize: '0.8rem', marginBottom: 4, paddingLeft: 8 }}>
                          {r.suggestion} — <span className="text-muted">{r.reasoning}</span>
                        </div>
                      ))}
                    </div>
                  )}

                  {aiRecs.warnings?.length > 0 && (
                    <div>
                      <h5 style={{ margin: '0 0 6px', fontSize: '0.85rem', color: 'var(--danger, #e53e3e)' }}>Warnings</h5>
                      {aiRecs.warnings.map((w: any, i: number) => (
                        <div key={i} style={{ fontSize: '0.8rem', marginBottom: 4, paddingLeft: 8 }}>
                          {w.warning} <span className="text-muted">({w.source})</span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}

          {error && (
            <div style={{
              background: 'var(--danger-bg)',
              color: 'var(--danger)',
              padding: '10px 14px',
              borderRadius: 'var(--radius-sm)',
              marginBottom: 12,
              fontSize: 14,
              border: '1px solid var(--danger)',
            }}>
              {error}
            </div>
          )}

          <div style={{ display: 'flex', gap: 10, marginTop: 8 }}>
            <button
              type="submit"
              className="btn btn-primary"
              disabled={createCampaign.isPending || !name.trim()}
            >
              {createCampaign.isPending ? 'Creating...' : 'Create Campaign'}
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => navigate('/')}
            >
              Cancel
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
