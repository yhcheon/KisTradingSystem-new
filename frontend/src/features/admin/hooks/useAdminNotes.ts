import { useCallback, useEffect, useState } from 'react';
import { adminNotesRequest } from '../api';
import type { Note } from '../types';

interface UseAdminNotesResult {
  notes: Note[];
  loading: boolean;
  message: string | null;
  setMessage: (message: string | null) => void;
  loadNotes: () => Promise<void>;
}

async function fetchNotes(): Promise<Note[]> {
  const result = await adminNotesRequest<{ data?: Note[] }>('GET');
  return Array.isArray(result?.data) ? result.data : [];
}

function getLoadErrorMessage(error: unknown): string {
  return `목록 조회 실패: ${error instanceof Error ? error.message : String(error)}`;
}

export function useAdminNotes(): UseAdminNotesResult {
  const [notes, setNotes] = useState<Note[]>([]);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);

  const loadNotes = useCallback(async () => {
    setLoading(true);
    try {
      setNotes(await fetchNotes());
    } catch (error) {
      setMessage(getLoadErrorMessage(error));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let active = true;

    fetchNotes()
      .then((result) => {
        if (active) setNotes(result);
      })
      .catch((error: unknown) => {
        if (active) setMessage(getLoadErrorMessage(error));
      })
      .finally(() => {
        if (active) setLoading(false);
      });

    return () => {
      active = false;
    };
  }, []);

  return { notes, loading, message, setMessage, loadNotes };
}
