import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useEffect, useRef } from 'react';
import { desktopAvailable } from './desktop';
import { example } from './example';
import { takeMenuRequest } from './shell';
import type { Workspace } from './useWorkspace';

export function useDesktopShell(
  w: Workspace,
  settings: () => void,
  newDownload: () => Promise<boolean>,
  hasDraft = false,
  prototype?: () => void,
) {
  const current = useRef({ w, settings, newDownload, hasDraft, prototype });
  useEffect(() => {
    current.current = { w, settings, newDownload, hasDraft, prototype };
  });

  useEffect(() => {
    if (!desktopAvailable) return;
    let disposed = false;
    let handling = false;
    let queued = false;
    let stop: (() => void) | undefined;
    async function receive() {
      if (disposed) return;
      if (handling) {
        queued = true;
        return;
      }
      handling = true;
      try {
        while (!disposed) {
          const request = await takeMenuRequest();
          if (!request || disposed) break;
          const { w, settings, newDownload, hasDraft } = current.current;
          if (hasDraft) {
            w.setError(
              'Save or close the current editor before opening another view.',
            );
          } else if (request.kind === 'settings') {
            settings();
          } else if (request.kind === 'authPrototype') {
            if (!w.busy) current.current.prototype?.();
          } else if (w.busy) {
            w.setError(
              'Finish the current operation before opening another album.',
            );
          } else if (await newDownload()) {
            if (request.kind === 'example') w.loadInspection(example, true);
          }
        }
      } catch (error) {
        if (!disposed) current.current.w.setError(String(error));
      } finally {
        handling = false;
        if (queued && !disposed) {
          queued = false;
          void receive();
        }
      }
    }
    // Listen before draining the native mailbox, including during startup.
    listen('jacket-menu-request', () => void receive())
      .then((unlisten) => {
        if (disposed) unlisten();
        else {
          stop = unlisten;
          void receive();
        }
      })
      .catch((error) => {
        if (!disposed) current.current.w.setError(String(error));
      });
    return () => {
      disposed = true;
      stop?.();
    };
  }, []);

  useEffect(() => {
    if (!desktopAvailable) return;
    invoke('workspace_status', { busy: w.busy }).catch((error) =>
      current.current.w.setError(String(error)),
    );
  }, [w.busy]);
}
