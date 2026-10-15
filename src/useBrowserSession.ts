import { useCallback, useRef, useState } from 'react';
import {
  type BrowserAccessCheck,
  type BrowserProfiles,
  browserProfiles,
  cancelExport,
  testBrowserAccess,
} from './desktop';
import type { Browser, ExportPlan } from './domain';

export interface BrowserSelection {
  browser: Browser | null;
  profileId: string | null;
}

export function useBrowserSession() {
  const [selection, setSelection] = useState<BrowserSelection>({
    browser: null,
    profileId: null,
  });
  const [catalog, setCatalog] = useState<{
    browser: Browser;
    value: BrowserProfiles;
  } | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<{
    browser: Browser;
    profileId: string;
    url: string;
    value: BrowserAccessCheck;
  } | null>(null);
  const refreshGeneration = useRef(0);
  const testingRef = useRef(false);
  const clearResult = useCallback(() => {
    setResult(null);
    setError(null);
  }, []);
  const attach = useCallback((browser: Browser, profileId: string) => {
    setSelection({ browser, profileId });
  }, []);
  const stop = useCallback(() => {
    setSelection({ browser: null, profileId: null });
    setResult(null);
    setError(null);
  }, []);
  const restore = useCallback((auth: ExportPlan['auth']) => {
    // Experimental owned sessions are never re-enabled in the normal flow.
    setSelection(
      auth.managedSession
        ? { browser: null, profileId: null }
        : { browser: auth.browser, profileId: auth.browserProfile ?? null },
    );
    setResult(null);
  }, []);
  const refresh = useCallback(async (browser: Browser) => {
    const generation = ++refreshGeneration.current;
    setRefreshing(true);
    setError(null);
    try {
      const value = await browserProfiles(browser);
      if (generation === refreshGeneration.current)
        setCatalog({ browser, value });
    } catch (error) {
      if (generation === refreshGeneration.current) {
        setCatalog(null);
        setError(String(error));
      }
    } finally {
      if (generation === refreshGeneration.current) setRefreshing(false);
    }
  }, []);
  async function test(browser: Browser, profileId: string, url: string) {
    if (testingRef.current) return;
    testingRef.current = true;
    setTesting(true);
    clearResult();
    try {
      const value = await testBrowserAccess(browser, profileId, url);
      setResult({ browser, profileId, url, value });
    } catch (error) {
      setError(String(error));
    } finally {
      testingRef.current = false;
      setTesting(false);
    }
  }
  async function cancelTest() {
    try {
      await cancelExport('browser-access');
    } catch (error) {
      setError(String(error));
    }
  }
  return {
    selection,
    attach,
    stop,
    restore,
    catalog,
    refresh,
    refreshing,
    testing,
    result,
    error,
    clearResult,
    test,
    cancelTest,
  };
}
