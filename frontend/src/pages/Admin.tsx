// ============================================================
// 파일명: pages/Admin.tsx
// 설명: 관리자 글 작성 / 목록 / 수정 / 삭제 페이지 조립 및 상태 관리
// ============================================================

import { useState } from 'react';
import { adminNotesRequest } from '../features/admin/api';
import type { Category, Note } from '../features/admin/types';
import { useAdminNotes } from '../features/admin/hooks/useAdminNotes';
import NewsManager from '../features/admin/NewsManager';
import AdminNoteForm from '../features/admin/AdminNoteForm';
import AdminNotesList from '../features/admin/AdminNotesList';

export default function Admin() {
  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');
  const [category, setCategory] = useState<Category>('us_market');
  const [saving, setSaving] = useState(false);
  const { notes, loading, message, setMessage, loadNotes } = useAdminNotes();
  const [editingId, setEditingId] = useState<string | number | null>(null);

  const resetForm = () => {
    setTitle('');
    setContent('');
    setCategory('us_market');
    setEditingId(null);
  };

  const handleSubmit = async () => {
    if (!title.trim() || !content.trim()) {
      setMessage('제목과 본문을 입력하세요.');
      return;
    }
    setSaving(true);
    setMessage(null);
    try {
      if (editingId !== null) {
        await adminNotesRequest('PUT', { id: String(editingId), title, content, category });
        setMessage('수정 완료');
      } else {
        await adminNotesRequest('POST', { title, content, category });
        setMessage('저장 완료');
      }
      resetForm();
      await loadNotes();
    } catch (error) {
      setMessage(`처리 실패: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setSaving(false);
    }
  };

  const startEdit = (note: Note) => {
    setEditingId(note.id);
    setTitle(note.title);
    setContent(note.content);
    setCategory(note.category === 'kospi_plan' ? 'kospi_plan' : 'us_market');
    setMessage(null);
    window.scrollTo({ top: 0, behavior: 'smooth' });
  };

  const handleDelete = async (note: Note) => {
    if (!window.confirm(`'${note.title}' 글을 삭제하시겠습니까? 삭제 후 복구할 수 없습니다.`)) return;
    setMessage(null);
    try {
      await adminNotesRequest('DELETE', { id: String(note.id) });
      if (editingId === note.id) resetForm();
      setMessage('삭제 완료');
      await loadNotes();
    } catch (error) {
      setMessage(`삭제 실패: ${error instanceof Error ? error.message : String(error)}`);
    }
  };

  return (
    <div style={{ padding: 24, maxWidth: 1000, margin: '0 auto', color: '#f8fafc' }}>
      <h2 style={{ marginBottom: 20 }}>관리자 글 관리</h2>

      <NewsManager />

      <AdminNoteForm
        title={title}
        content={content}
        category={category}
        saving={saving}
        editing={editingId !== null}
        message={message}
        onTitleChange={setTitle}
        onContentChange={setContent}
        onCategoryChange={setCategory}
        onSubmit={() => void handleSubmit()}
        onCancel={resetForm}
      />

      <AdminNotesList
        notes={notes}
        loading={loading}
        onRefresh={() => void loadNotes()}
        onEdit={startEdit}
        onDelete={(note) => void handleDelete(note)}
      />
    </div>
  );
}
