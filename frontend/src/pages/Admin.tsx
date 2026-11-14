// ============================================================
// 파일명: pages/Admin.tsx
// 설명: 관리자 글 작성 페이지
// 작성일: 2026-11-14
// ============================================================

import { useState } from 'react';
import { supabase } from '../lib/supabase';

export default function Admin() {
  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');
  const [category, setCategory] = useState<'us_market' | 'kospi_plan'>('us_market');
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const handleSubmit = async () => {
    if (!title.trim() || !content.trim()) {
      setMessage('제목과 본문을 입력하세요');
      return;
    }
    setSaving(true);
    setMessage(null);

    try {
      const { data: session } = await supabase.auth.getSession();
      const token = session.session?.access_token;
      if (!token) { setMessage('로그인 필요'); setSaving(false); return; }

      const res = await fetch('/api/admin/notes', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`,
        },
        body: JSON.stringify({ title, content, category }),
      });

      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        setMessage(`저장 실패: ${err.error || res.status}`);
        setSaving(false);
        return;
      }

      setMessage('저장 완료');
      setTitle('');
      setContent('');
    } catch (e) {
      setMessage(`오류: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div style={{ padding: 24, maxWidth: 900, margin: '0 auto' }}>
      <h2 style={{ color: '#f8fafc', marginBottom: 20 }}>관리자 글 작성</h2>

      <div style={{ marginBottom: 12 }}>
        <label style={{ color: '#94a3b8', display: 'block', marginBottom: 6 }}>카테고리</label>
        <select
          value={category}
          onChange={(e) => setCategory(e.target.value as 'us_market' | 'kospi_plan')}
          style={{
            background: '#1e293b', color: '#e2e8f0',
            border: '1px solid #334155', borderRadius: 6,
            padding: '8px 12px', fontSize: 14,
          }}
        >
          <option value="us_market">미국장 분석</option>
          <option value="kospi_plan">코스피 대응</option>
        </select>
      </div>

      <div style={{ marginBottom: 12 }}>
        <label style={{ color: '#94a3b8', display: 'block', marginBottom: 6 }}>제목</label>
        <input
          type="text"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="제목을 입력하세요"
          style={{
            width: '100%', background: '#1e293b', color: '#e2e8f0',
            border: '1px solid #334155', borderRadius: 6,
            padding: '10px 12px', fontSize: 14,
          }}
        />
      </div>

      <div style={{ marginBottom: 12 }}>
        <label style={{ color: '#94a3b8', display: 'block', marginBottom: 6 }}>
          본문 (Markdown)
        </label>
        <textarea
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder="## 소제목&#10;&#10;- 항목1&#10;- 항목2&#10;&#10;**굵게** 표시도 가능"
          rows={15}
          style={{
            width: '100%', background: '#1e293b', color: '#e2e8f0',
            border: '1px solid #334155', borderRadius: 6,
            padding: '10px 12px', fontSize: 14,
            fontFamily: 'Consolas, monospace',
          }}
        />
      </div>

      <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
        <button
          onClick={handleSubmit}
          disabled={saving}
          style={{
            background: '#3b82f6', color: 'white',
            border: 'none', borderRadius: 6,
            padding: '10px 20px', fontSize: 14,
            cursor: saving ? 'wait' : 'pointer',
            opacity: saving ? 0.6 : 1,
          }}
        >
          {saving ? '저장 중...' : '저장'}
        </button>
        {message && (
          <span style={{ color: message.includes('완료') ? '#10b981' : '#ef4444', fontSize: 13 }}>
            {message}
          </span>
        )}
      </div>
    </div>
  );
}

