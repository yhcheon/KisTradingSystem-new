import { useEffect, useState } from 'react';

export type BackendHealthStatus = 'checking' | 'ready' | 'unavailable' | 'failed';

interface HealthResponse {
  status?: string;
  ready?: boolean;
}

const INITIAL_RETRY_DELAY_MS = 1_000;
const MAX_RETRY_DELAY_MS = 5_000;
const READY_POLL_INTERVAL_MS = 3_000;

export function useBackendHealth(): BackendHealthStatus {
  const [status, setStatus] = useState<BackendHealthStatus>('checking');

  useEffect(() => {
    let disposed = false;
    let timer: number | null = null;
    let retryDelay = INITIAL_RETRY_DELAY_MS;
    let controller: AbortController | null = null;

    const checkHealth = async () => {
      controller = new AbortController();

      try {
        const response = await fetch('/api/health', {
          method: 'GET',
          cache: 'no-store',
          signal: controller.signal,
        });
        const payload: HealthResponse = await response.json();

        if (disposed) return;

        if (response.ok && (payload.ready === true || payload.status === 'ok')) {
          setStatus('ready');
          retryDelay = INITIAL_RETRY_DELAY_MS;
          timer = window.setTimeout(checkHealth, READY_POLL_INTERVAL_MS);
          return;
        }

        if (payload.status === 'failed') {
          setStatus('failed');
          retryDelay = MAX_RETRY_DELAY_MS;
        } else {
          setStatus('checking');
          retryDelay = INITIAL_RETRY_DELAY_MS;
        }
      } catch {
        if (disposed) return;
        setStatus('unavailable');
        retryDelay = Math.min(retryDelay * 2, MAX_RETRY_DELAY_MS);
      }

      if (!disposed) {
        timer = window.setTimeout(checkHealth, retryDelay);
      }
    };

    void checkHealth();

    return () => {
      disposed = true;
      if (timer !== null) window.clearTimeout(timer);
      controller?.abort();
    };
  }, []);

  return status;
}
