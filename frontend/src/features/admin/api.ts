import { supabase } from '../../lib/supabase';

export async function adminNotesRequest<T = unknown>(
  method: string,
  body?: object,
): Promise<T | null> {
  const { data: sessionData, error: sessionError } = await supabase.auth.getSession();
  if (sessionError) throw sessionError;

  const token = sessionData.session?.access_token;
  if (!token) throw new Error('로그인이 필요합니다.');

  const response = await fetch('/api/admin/notes', {
    method,
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    ...(body ? { body: JSON.stringify(body) } : {}),
  });

  if (!response.ok) {
    const detail = await response.text();
    throw new Error(detail || `HTTP ${response.status}`);
  }
  if (response.status === 204) return null;
  return (await response.json()) as T;
}
