import { API_BASE } from '../lib/constants';

export async function apiFetch<T>(path: string, options?: RequestInit): Promise<T> {
  const method = options?.method || 'GET';
  console.log(`[API] ${method} ${path}`, options?.body ? JSON.parse(options.body as string) : '');

  let res: Response;
  try {
    res = await fetch(`${API_BASE}${path}`, {
      headers: { 'Content-Type': 'application/json', ...options?.headers },
      ...options,
    });
  } catch (err) {
    console.error(`[API] ${method} ${path} — network error:`, err);
    throw new Error(`Network error: Could not reach backend at ${API_BASE}. Is it running?`);
  }

  if (!res.ok) {
    let detail = '';
    try {
      const body = await res.json();
      detail = body.detail ? (typeof body.detail === 'string' ? body.detail : JSON.stringify(body.detail)) : JSON.stringify(body);
    } catch {
      detail = await res.text().catch(() => '');
    }
    console.error(`[API] ${method} ${path} → ${res.status}: ${detail}`);
    throw new Error(detail || `API error: ${res.status}`);
  }

  const data = await res.json();
  console.log(`[API] ${method} ${path} → ${res.status}`, data);
  return data;
}
