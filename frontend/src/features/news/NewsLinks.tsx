import { useEffect, useState } from 'react';
import { authenticatedFetch } from '../../lib/apiClient';
import type { NewsLink } from '../admin/types';

export default function NewsLinks() {
  const [links, setLinks] = useState<NewsLink[]>([]);
  const [message, setMessage] = useState('뉴스를 불러오는 중...');

  useEffect(() => {
    let active = true;
    const load = async () => {
      const response = await authenticatedFetch('/api/news');
      if (!response) {
        if (active) setMessage('로그인이 필요합니다.');
        return;
      }
      if (response.status === 403) {
        if (active) setMessage('무료 회원은 오후 2시부터 열람 가능합니다.');
        return;
      }
      if (!response.ok) {
        if (active) setMessage('뉴스 목록을 불러올 수 없습니다.');
        return;
      }
      const result = await response.json() as { data?: NewsLink[] };
      if (active) {
        const data = Array.isArray(result.data) ? result.data : [];
        setLinks(data);
        setMessage(data.length ? '' : '등록된 뉴스가 없습니다.');
      }
    };
    void load().catch(() => {
      if (active) setMessage('뉴스 목록을 불러올 수 없습니다.');
    });
    return () => { active = false; };
  }, []);

  return (
    <section style={{ background: '#1e293b', padding: 20, borderRadius: 12, margin: '0 20px 20px' }}>
      <h3 style={{ color: '#f8fafc', marginTop: 0 }}>주요 뉴스</h3>
      {message && <p style={{ color: '#94a3b8' }}>{message}</p>}
      {links.map(link => (
        <article key={link.id} style={{ borderTop: '1px solid #334155', padding: '10px 0' }}>
          <a href={link.url} target="_blank" rel="noreferrer" style={{ color: '#93c5fd', overflowWrap: 'anywhere' }}>{link.title}</a>
          <div style={{ color: '#94a3b8', fontSize: 12 }}>
            {link.source || '출처 미지정'}{link.published_at ? ` · ${new Date(link.published_at).toLocaleString('ko-KR')}` : ''}
          </div>
        </article>
      ))}
    </section>
  );
}
