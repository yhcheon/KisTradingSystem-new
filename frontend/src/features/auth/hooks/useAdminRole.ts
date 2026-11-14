import { useEffect, useState } from 'react';
import type { Session } from '@supabase/supabase-js';
import { supabase } from '../../../lib/supabase';

export function useAdminRole(session: Session | null): boolean {
  const [adminUserId, setAdminUserId] = useState<string | null>(null);

  useEffect(() => {
    if (!session) return;

    let active = true;
    const userId = session.user.id;

    const checkAdmin = async () => {
      const { data } = await supabase
        .from('user_profiles')
        .select('tier')
        .eq('user_id', userId)
        .maybeSingle();

      if (active) {
        setAdminUserId(data?.tier === 'admin' ? userId : null);
      }
    };

    void checkAdmin();

    return () => {
      active = false;
    };
  }, [session]);

  return session !== null && adminUserId === session.user.id;
}
