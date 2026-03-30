import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Plus, ArrowLeft, Package } from 'lucide-react';
import { useCreateCampaign } from '../hooks/useCampaigns';
import { useActiveProfile } from '../hooks/useActiveProfile';
import { apiFetch } from '../hooks/useApi';
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
  const [audience, setAudience] = useState('');
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
        target_audience: audience || undefined,
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
            <input
              className="form-input"
              type="text"
              value={audience}
              onChange={(e) => setAudience(e.target.value)}
              placeholder="e.g., Indie developers, content creators"
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
