import { useCallback, useEffect, useState } from 'react';
import {
  desktopAvailable,
  type SessionStatus,
  sessionAction,
  sessionStatus,
} from './desktop';
import { readPreference, writePreference } from './preferences';

export function useOwnedSession() {
  const [status, setStatus] = useState<SessionStatus | null>(null);
  const [selected, setSelected] = useState<string | null>(
    () => readPreference('owned-session') || null,
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const select = useCallback((id: string | null) => {
    setSelected(id);
    writePreference('owned-session', id ?? '');
  }, []);
  useEffect(() => {
    if (!desktopAvailable) return;
    let disposed = false;
    sessionStatus()
      .then((value) => {
        if (disposed) return;
        setStatus(value);
        if (
          !value.supported ||
          readPreference('owned-session') !== value.sessionId
        )
          select(null);
      })
      .catch((error) => {
        if (!disposed) {
          setError(String(error));
          setStatus({
            supported: true,
            sessionId: null,
            signingIn: false,
            verified: false,
            message:
              'The saved session could not be read. Disconnect to reset it.',
          });
        }
      });
    return () => {
      disposed = true;
    };
  }, [select]);
  async function act(
    action: Parameters<typeof sessionAction>[0],
    url?: string,
  ) {
    if (busy) return;
    setBusy(true);
    setError(null);
    if (action === 'connect' || action === 'disconnect') select(null);
    try {
      const value = await sessionAction(action, url ?? null);
      setStatus(value);
      if (action === 'save') select(value.sessionId);
    } catch (error) {
      setError(String(error));
      const value = await sessionStatus().catch(() => null);
      if (value) {
        setStatus(value);
        if (!value.sessionId) select(null);
      }
    } finally {
      setBusy(false);
    }
  }
  return { status, selected, select, busy, error, act };
}
