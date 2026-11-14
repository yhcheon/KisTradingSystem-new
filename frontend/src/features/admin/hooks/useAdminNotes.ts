import { useCallback, useEffect, useState } from 'react';
import { adminNotesRequest } from '../api';
import type { Note } from '../types';

interface UseAdminNotesResult {
  notes: Note[];
  loading: boolean;
  message: string | null;
  setMessage: (message: string | null) => void;
  loadNotes: (showLoading?: boolean) => Promise<void>;
}

export function useAdminNotes(): UseAdminNotesResult {
  const [notes, setNotes] = useState<Note[]>([]);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);

  const loadNotes = useCallback(async (showLoading = true) => {
    if (showLoading) setLoading(true);
    try {
      const result = await adminNotesRequest<{ data?: Note[] }>('GET');
      setNotes(Array.isArray(result?.data) ? result.data : []);
    } catch (error) {
      setMessage(`목록 조회 실패: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadNotes(false);
  }, [loadNotes]);

  return { notes, loading, message, setMessage, loadNotes };
}
