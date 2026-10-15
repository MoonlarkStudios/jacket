import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useRef, useState } from 'react';
import {
  checkDownloaderUpdate,
  configureDownloader,
  desktopAvailable,
  restoreDownloader,
  cancelDownloaderUpdate as stopUpdate,
  updateDownloader,
} from './desktop';
import type {
  DownloaderChannel,
  DownloaderPreferences,
  DownloaderStatus,
} from './domain';
import { automaticUpdateKey, type DownloaderPhase } from './downloader';

export function useDownloader(jobBusy: boolean) {
  const [downloader, setDownloader] = useState<DownloaderStatus | null>(null);
  const [checkingDownloader, setCheckingDownloader] =
    useState(desktopAvailable);
  const [updatingDownloader, setUpdatingDownloader] = useState(false);
  const [configuringDownloader, setConfiguringDownloader] = useState(false);
  const [cancellingDownloader, setCancellingDownloader] = useState(false);
  const [downloaderPhase, setDownloaderPhase] = useState<DownloaderPhase>(
    desktopAvailable ? 'checking' : null,
  );
  const [downloaderError, setDownloaderError] = useState<string | null>(null);
  const startup = useRef<Promise<DownloaderStatus> | null>(null);
  const inFlight = useRef(false);
  const attempted = useRef<string | null>(null);
  const pausedForSession = useRef(false);

  useEffect(() => {
    if (!desktopAvailable) return;
    let disposed = false;
    // Reuse this promise across React StrictMode's effect replay.
    startup.current ??= checkDownloaderUpdate();
    startup.current
      .then((value) => {
        if (!disposed) setDownloader(value);
      })
      .catch((error) => {
        if (!disposed) setDownloaderError(String(error));
      })
      .finally(() => {
        if (!disposed) {
          setCheckingDownloader(false);
          setDownloaderPhase(null);
        }
      });
    let unsubscribe: (() => void) | undefined;
    listen<string>('downloader-progress', ({ payload }) => {
      if (
        !disposed &&
        ['checking', 'downloading', 'verifying'].includes(payload)
      )
        setDownloaderPhase(payload as DownloaderPhase);
    })
      .then((stop) => {
        if (disposed) stop();
        else unsubscribe = stop;
      })
      .catch(() => {
        /* The operation still returns its final status. */
      });
    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, []);

  const installDownloader = useCallback(async () => {
    if (inFlight.current || checkingDownloader || jobBusy || !downloader)
      return;
    inFlight.current = true;
    attempted.current = automaticUpdateKey(downloader);
    setUpdatingDownloader(true);
    setDownloaderPhase('checking');
    setDownloaderError(null);
    try {
      const value = await updateDownloader(downloader.channel);
      setDownloader(value);
      if (value.pendingActivation) attempted.current = null;
    } catch (error) {
      setDownloaderError(
        String(error).startsWith('Cancelled')
          ? 'Update stopped. Your current version is unchanged.'
          : 'Couldn’t update yt-dlp. Your current version is still available. Check again to retry.',
      );
    } finally {
      inFlight.current = false;
      setUpdatingDownloader(false);
      setCancellingDownloader(false);
      setDownloaderPhase(null);
    }
  }, [checkingDownloader, jobBusy, downloader]);

  useEffect(() => {
    const key = automaticUpdateKey(downloader);
    if (
      !desktopAvailable ||
      !key ||
      jobBusy ||
      checkingDownloader ||
      updatingDownloader ||
      configuringDownloader ||
      inFlight.current ||
      pausedForSession.current ||
      attempted.current === key
    )
      return;
    attempted.current = key;
    void installDownloader();
  }, [
    downloader,
    jobBusy,
    checkingDownloader,
    updatingDownloader,
    configuringDownloader,
    installDownloader,
  ]);

  async function checkDownloader(channel: DownloaderChannel | null = null) {
    if (inFlight.current || checkingDownloader) return;
    inFlight.current = true;
    setCheckingDownloader(true);
    setDownloaderPhase('checking');
    setDownloaderError(null);
    pausedForSession.current = false;
    attempted.current = null;
    try {
      setDownloader(await checkDownloaderUpdate(true, channel));
    } catch (error) {
      setDownloaderError(String(error));
    } finally {
      inFlight.current = false;
      setCheckingDownloader(false);
      setDownloaderPhase(null);
    }
  }

  async function setDownloaderPreferences(preferences: DownloaderPreferences) {
    if (jobBusy || inFlight.current || checkingDownloader) return false;
    inFlight.current = true;
    setConfiguringDownloader(true);
    setDownloaderError(null);
    try {
      setDownloader(await configureDownloader(preferences));
      attempted.current = null;
      pausedForSession.current = false;
      return true;
    } catch (error) {
      setDownloaderError(String(error));
      return false;
    } finally {
      inFlight.current = false;
      setConfiguringDownloader(false);
    }
  }

  async function restoreBundledDownloader() {
    if (jobBusy || inFlight.current || checkingDownloader) return;
    inFlight.current = true;
    setConfiguringDownloader(true);
    setDownloaderError(null);
    try {
      setDownloader(await restoreDownloader());
      attempted.current = null;
    } catch (error) {
      setDownloaderError(String(error));
    } finally {
      inFlight.current = false;
      setConfiguringDownloader(false);
    }
  }

  async function cancelDownloaderUpdate() {
    pausedForSession.current = true;
    setCancellingDownloader(true);
    try {
      await stopUpdate();
    } catch (error) {
      setDownloaderError(String(error));
      setCancellingDownloader(false);
    }
  }

  return {
    downloader,
    checkingDownloader,
    updatingDownloader,
    configuringDownloader,
    cancellingDownloader,
    downloaderPhase,
    downloaderError,
    checkDownloader,
    installDownloader,
    setDownloaderPreferences,
    restoreBundledDownloader,
    cancelDownloaderUpdate,
  };
}
