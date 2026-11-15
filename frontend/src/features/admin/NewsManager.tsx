import { useCallback, useEffect, useState } from 'react';
import { adminNewsRequest } from './api';
import type { NewsLink } from './types';

const panelStyle = {
  background: '#111827', border: '1px solid #334155', borderRadius: 8,
  padding: 16, marginBottom: 12, color: '#e2e8f0',
};
const inputStyle = {
  width: '100%', boxSizing: 'border-box' as const, background: '#1e293b',
  color: '#e2e8f0', border: '1px solid #334155', borderRadius: 6,
  padding: '10px 12px', fontSize: 14,
};

export default function NewsManager() {
  const [title, setTitle] = useState('');
  const [url, setUrl] = useState('');
  const [source, setSource] = useState('');
  const [links, setLinks] = useState<NewsLink[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const loadLinks = useCallback(async () => {
    setLoading(true);
    try {
      const result = await adminNewsRequest<{ data?: NewsLink[] }>('GET');
      setLinks(Array.isArray(result?.data) ? result.data : []);
    } catch (error) {
      setMessage(`뉴스 목록 조회 실패: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void loadLinks(); }, [loadLinks]);

  const handleSubmit = async () => {
    if (!title.trim() || !url.trim()) {
      setMessage('뉴스 제목과 URL을 입력하세요.');
      return;
    }
    try {
      const parsed = new URL(url);
      if (!['http:', 'https:'].includes(parsed.protocol)) throw new Error('invalid protocol');
    } catch {
      setMessage('http 또는 https 형식의 올바른 URL을 입력하세요.');
      return;
    }

    setSaving(true);
    setMessage(null);
    try {
      await adminNewsRequest('POST', { title: title.trim(), url: url.trim(), source: source.trim() });
      setTitle('');
      setUrl('');
      setSource('');
      setMessage('뉴스 링크 등록 완료');
      await loadLinks();
    } catch (error) {
      setMessage(`등록 실패: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (link: NewsLink) => {
    if (!window.confirm(`'${link.title}' 뉴스 링크를 삭제하시겠습니까?`)) return;
    setMessage(null);
    try {
      await adminNewsRequest('DELETE', { id: link.id });
      setMessage('뉴스 링크 삭제 완료');
      await loadLinks();
    } catch (error) {
      setMessage(`삭제 실패: ${error instanceof Error ? error.message : String(error)}`);
    }
  };

  return (
    <section style={panelStyle}>
      <h3 style={{ marginTop: 0 }}>뉴스 링크 관리</h3>
      <div style={{ display: 'grid', gap: 10 }}>
        <input value={title} onChange={event => setTitle(event.target.value)} placeholder="뉴스 제목" style={inputStyle} />
        <input value={url} onChange={event => setUrl(event.target.value)} placeholder="https://뉴스 URL" type="url" style={inputStyle} />
        <input value={source} onChange={event => setSource(event.target.value)} placeholder="언론사 (선택)" style={inputStyle} />
        <button onClick={() => void handleSubmit()} disabled={saving}
          style={{ background: '#2563eb', color: 'white', border: 0, borderRadius: 6, padding: '10px 14px', cursor: 'pointer' }}>
          {saving ? '등록 중...' : '뉴스 링크 등록'}
        </button>
      </div>
      {message && <p role="status" style={{ color: message.includes('완료') ? '#10b981' : '#f87171', fontSize: 13 }}>{message}</p>}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 20 }}>
        <h4>등록된 뉴스 ({links.length})</h4>
        <button onClick={() => void loadLinks()} disabled={loading}>새로고침</button>
      </div>
      {loading && <p style={{ color: '#94a3b8' }}>뉴스를 불러오는 중...</p>}
      {!loading && links.length === 0 && <p style={{ color: '#94a3b8' }}>등록된 뉴스가 없습니다.</p>}
      {links.map(link => (
        <article key={link.id} style={{ borderTop: '1px solid #334155', padding: '12px 0', display: 'flex', gap: 12, justifyContent: 'space-between' }}>
          <div style={{ minWidth: 0 }}>
            <a href={link.url} target="_blank" rel="noreferrer" style={{ color: '#93c5fd', overflowWrap: 'anywhere' }}>{link.title}</a>
            <div style={{ color: '#94a3b8', fontSize: 12 }}>{link.source || '출처 미지정'}{link.published_at ? ` · ${new Date(link.published_at).toLocaleString('ko-KR')}` : ''}</div>
          </div>
          <button onClick={() => void handleDelete(link)} style={{ alignSelf: 'flex-start', background: '#b91c1c', color: 'white', border: 0, borderRadius: 6, padding: '7px 10px', cursor: 'pointer' }}>삭제</button>
        </article>
      ))}
    </section>
  );
}
