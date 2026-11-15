import { authenticatedFetch } from '../../lib/apiClient';

export async function adminNotesRequest<T = unknown>(
  method: string,
  body?: object,
): Promise<T | null> {
  const response = await authenticatedFetch('/api/admin/notes', {
    method,
    headers: { 'Content-Type': 'application/json' },
    ...(body ? { body: JSON.stringify(body) } : {}),
  });

  if (!response) throw new Error('로그인이 필요합니다.');

  if (!response.ok) {
    const detail = await response.text();
    throw new Error(detail || `HTTP ${response.status}`);
  }
  if (response.status === 204) return null;
  return (await response.json()) as T;
}

export async function adminNewsRequest<T = unknown>(
  method: string,
  body?: object,
): Promise<T | null> {
  const response = await authenticatedFetch('/api/admin/news', {
    method,
    headers: { 'Content-Type': 'application/json' },
    ...(body ? { body: JSON.stringify(body) } : {}),
  });
  if (!response) throw new Error('로그인이 필요합니다.');
  if (!response.ok) {
    const detail = await response.text();
    throw new Error(detail || `HTTP ${response.status}`);
  }
  if (response.status === 204) return null;
  return (await response.json()) as T;
}
