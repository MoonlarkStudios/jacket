import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useRef, useState } from 'react';
import {
  archiveDestination,
  cancelExport,
  cancelInspection,
  chooseArchive,
  dependencyStatus,
  desktopAvailable,
  dismissRecovery,
  inspectSource,
  pendingExport,
  startExport,
} from './desktop';
import {
  type CollectionMode,
  type ExportPlan,
  type Inspection,
  type InspectionEvent,
  type JobEvent,
  type PendingExport,
  selectedTracks,
  type Track,
} from './domain';
import {
  audioFormat,
  type ExportPreferences,
  inspectionPreferences,
  numberAlbumTracks,
  readPreference,
  recoveryPreferences,
  updateAlbumPreferences,
  withAlbumTrackNumbers,
  writePreference,
} from './preferences';
import { useBrowserSession } from './useBrowserSession';
import { useDownloader } from './useDownloader';

const defaults: ExportPreferences = {
  title: '',
  mode: 'collection',
  albumTitle: '',
  albumArtist: '',
  year: null,
  format: 'compatible',
  artworkMode: 'perTrack',
  sharedArtwork: null,
};

interface ReviewSnapshot {
  url: string;
  inspection: Inspection;
  selected: Set<number>;
  preferences: ExportPreferences;
  inspectionTotal: number | null;
  progress: JobEvent | null;
  savePath: string | null;
  error: string | null;
  exampleMode: boolean;
  numberingUndo: (number | null)[] | null;
  currentJob: string | null;
  retryOutput: string | null;
  retryTitle: string | null;
  auth: ExportPlan['auth'];
}

export function useWorkspace() {
  const [url, setUrl] = useState('');
  const browserSession = useBrowserSession();
  const { browser, profileId: browserProfile } = browserSession.selection;
  const [inspection, setInspection] = useState<Inspection | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [preferences, setPreferences] = useState(() => ({
    ...defaults,
    format: audioFormat(readPreference('format')),
  }));
  const [loading, setLoading] = useState(false);
  const [inspectionStatus, setInspectionStatus] = useState<
    'idle' | 'reading' | 'complete' | 'cancelled' | 'failed'
  >('idle');
  const [inspectionTotal, setInspectionTotal] = useState<number | null>(null);
  const [exporting, setExporting] = useState(false);
  const [choosing, setChoosing] = useState(false);
  const [resetting, setResetting] = useState(false);
  const [previousReview, setPreviousReview] = useState<ReviewSnapshot | null>(
    null,
  );
  const [savePath, setSavePath] = useState<string | null>(null);
  const [cancelling, setCancelling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<JobEvent | null>(null);
  const [eventsReady, setEventsReady] = useState(!desktopAvailable);
  const downloaderState = useDownloader(
    loading || exporting || choosing || browserSession.testing,
  );
  const [dependencies, setDependencies] = useState<Array<{
    name: string;
    available: boolean;
    bundled?: boolean;
  }> | null>(null);
  const [exampleMode, setExampleMode] = useState(false);
  const [numberingUndo, setNumberingUndo] = useState<(number | null)[] | null>(
    null,
  );
  const currentJob = useRef<string | null>(null);
  const choosingRef = useRef(false);
  const resettingRef = useRef(false);
  const currentInspection = useRef<string | null>(null);
  const inspectionCancellation = useRef(false);
  const retryOutput = useRef<string | null>(null);
  const retryTitle = useRef<string | null>(null);
  const savedFormat = useRef(preferences.format);
  const userStarted = useRef(false);

  const restorePendingExport = useCallback(
    (value: PendingExport) => {
      const plan = value.plan;
      setInspection({
        title: plan.title,
        sourceUrl: plan.sourceUrl,
        suggestedMode: plan.mode,
        artworkUrl:
          plan.sharedArtworkUrl ||
          plan.tracks.find((track) => track.artworkUrl)?.artworkUrl ||
          null,
        tracks: plan.tracks,
        warnings: [],
      });
      setSelected(new Set(plan.tracks.map((_, index) => index)));
      setPreferences(recoveryPreferences(plan));
      browserSession.restore(plan.auth);
      setUrl(plan.sourceUrl);
      setInspectionStatus('complete');
      setInspectionTotal(plan.tracks.length);
      currentJob.current = value.jobId;
      retryOutput.current = value.outputPath;
      retryTitle.current = plan.title;
      setSavePath(value.outputPath);
      setProgress({
        jobId: value.jobId,
        stage: 'interrupted',
        message: 'Ready to resume',
        completed: value.progress.completed,
        total: plan.tracks.length,
        result: null,
        error: value.error,
      });
    },
    [browserSession.restore],
  );

  useEffect(() => {
    if (exampleMode || inspectionStatus !== 'complete') return;
    savedFormat.current = preferences.format;
    writePreference('format', preferences.format);
  }, [preferences.format, exampleMode, inspectionStatus]);

  useEffect(() => {
    if (!desktopAvailable || exampleMode || inspectionStatus !== 'complete')
      return;
    let disposed = false;
    if (
      currentJob.current &&
      progress?.stage !== 'complete' &&
      retryOutput.current &&
      retryTitle.current === preferences.title
    ) {
      setSavePath(retryOutput.current);
      return;
    }
    setSavePath(null);
    archiveDestination(preferences.title)
      .then((path) => {
        if (!disposed) setSavePath(path);
      })
      .catch(() => {
        if (!disposed) setSavePath(null);
      });
    return () => {
      disposed = true;
    };
  }, [preferences.title, exampleMode, inspectionStatus, progress?.stage]);

  useEffect(() => {
    if (!desktopAvailable) return;
    let disposed = false;
    dependencyStatus()
      .then((value) => {
        if (!disposed) setDependencies(value);
      })
      .catch((error) => {
        if (!disposed) setError(String(error));
      });
    pendingExport()
      .then((value) => {
        if (!disposed && !userStarted.current && value)
          restorePendingExport(value);
      })
      .catch(() => {
        /* An unavailable recovery record never blocks a new download. */
      });
    let unsubscribe: (() => void) | undefined;
    listen<JobEvent>('export-progress', ({ payload }) => {
      if (disposed || payload.jobId !== currentJob.current) return;
      setProgress(payload);
      if (['complete', 'failed', 'cancelled'].includes(payload.stage)) {
        setExporting(false);
        setCancelling(false);
      }
    })
      .then((stop) => {
        if (disposed) stop();
        else {
          unsubscribe = stop;
          setEventsReady(true);
        }
      })
      .catch((error) => {
        if (!disposed) setError(String(error));
      });
    return () => {
      disposed = true;
      currentInspection.current = null;
      unsubscribe?.();
    };
  }, [restorePendingExport]);

  const busy =
    loading || exporting || choosing || resetting || browserSession.testing;
  const toolsReady =
    dependencies?.every((tool) => tool.available) ?? !desktopAvailable;
  const chosen = inspection
    ? selectedTracks(inspection.tracks, selected, preferences.mode, {
        title: preferences.albumTitle,
        artist: preferences.albumArtist,
        year: preferences.year,
      })
    : [];

  function loadInspection(value: Inspection, isExample = false) {
    if (resettingRef.current) return;
    userStarted.current = true;
    setPreviousReview(null);
    const prepared = withAlbumTrackNumbers(value);
    setNumberingUndo(null);
    setInspection(prepared);
    setSelected(new Set(value.tracks.map((_, index) => index)));
    setPreferences(inspectionPreferences(value, savedFormat.current));
    setInspectionStatus('complete');
    setInspectionTotal(value.tracks.length);
    setProgress(null);
    setSavePath(null);
    currentJob.current = null;
    retryOutput.current = null;
    retryTitle.current = null;
    setError(null);
    setExampleMode(isExample);
  }

  function changeUrl(value: string) {
    userStarted.current = true;
    setUrl(value);
  }

  async function inspect(source = url.trim()) {
    if (
      busy ||
      resettingRef.current ||
      choosingRef.current ||
      currentInspection.current ||
      !source
    )
      return;
    userStarted.current = true;
    setUrl(source);
    const id = crypto.randomUUID();
    currentInspection.current = id;
    inspectionCancellation.current = false;
    setError(null);
    setLoading(true);
    setCancelling(false);
    setInspection(null);
    setSavePath(null);
    setSelected(new Set());
    setInspectionStatus('reading');
    setInspectionTotal(null);
    setProgress(null);
    setExampleMode(false);
    currentJob.current = null;
    let unsubscribe: (() => void) | undefined;
    const streamedTracks = new Map<number, Track>();
    try {
      // Subscribe before starting the native job so the first fast result
      // cannot arrive before the listener. Scope events to this inspection.
      unsubscribe = await listen<InspectionEvent>(
        'inspection-progress',
        ({ payload }) => {
          if (payload.inspectionId !== currentInspection.current) return;
          if (inspectionCancellation.current) return;
          const first = streamedTracks.size === 0;
          streamedTracks.set(payload.index, payload.track);
          const preview: Inspection = withAlbumTrackNumbers({
            title: payload.title,
            sourceUrl: payload.sourceUrl,
            suggestedMode: payload.suggestedMode,
            artworkUrl: payload.artworkUrl,
            tracks: [...streamedTracks.entries()]
              .sort(([a], [b]) => a - b)
              .map(([, track]) => track),
            warnings: [],
          });
          if (first)
            setPreferences(inspectionPreferences(preview, savedFormat.current));
          setInspection(preview);
          setSelected(new Set(preview.tracks.map((_, index) => index)));
          setInspectionTotal(payload.total);
        },
      );
      if (currentInspection.current !== id) return;
      if (inspectionCancellation.current) {
        setInspectionStatus('cancelled');
        return;
      }
      const result = await inspectSource(source, browser, id, browserProfile);
      if (currentInspection.current === id) loadInspection(result);
    } catch (error) {
      if (currentInspection.current !== id) return;
      const cancelled = String(error).startsWith('Cancelled');
      setInspectionStatus(cancelled ? 'cancelled' : 'failed');
      if (!cancelled) setError(String(error));
    } finally {
      unsubscribe?.();
      if (currentInspection.current === id) {
        currentInspection.current = null;
        setLoading(false);
        setCancelling(false);
      }
    }
  }

  async function exportZip() {
    if (
      !inspection ||
      inspectionStatus !== 'complete' ||
      chosen.length === 0 ||
      !preferences.title.trim() ||
      busy ||
      resettingRef.current ||
      currentInspection.current ||
      choosingRef.current ||
      !eventsReady ||
      !desktopAvailable ||
      exampleMode
    )
      return;
    choosingRef.current = true;
    setChoosing(true);
    setError(null);
    try {
      const retrying = ['failed', 'cancelled', 'interrupted'].includes(
        progress?.stage ?? '',
      );
      const path =
        retrying &&
        retryOutput.current &&
        retryTitle.current === preferences.title
          ? retryOutput.current
          : await chooseArchive(preferences.title);
      if (!path) return;
      setSavePath(path);
      retryOutput.current = path;
      retryTitle.current = preferences.title;
      const id =
        currentJob.current &&
        ['failed', 'cancelled', 'interrupted'].includes(progress?.stage ?? '')
          ? currentJob.current
          : crypto.randomUUID();
      currentJob.current = id;
      const queued: JobEvent = {
        jobId: id,
        stage: 'queued',
        message: 'Starting your download',
        completed: 0,
        total: chosen.length,
        result: null,
        error: null,
      };
      setProgress(queued);
      setExporting(true);
      setCancelling(false);
      try {
        await startExport(
          {
            title: preferences.title,
            sourceUrl: inspection.sourceUrl,
            mode: preferences.mode,
            format: preferences.format,
            artworkMode: preferences.artworkMode,
            sharedArtworkUrl: preferences.sharedArtwork,
            auth: {
              browser,
              browserProfile,
            },
            tracks: chosen,
          },
          path,
          id,
        );
      } catch (error) {
        setExporting(false);
        setProgress({
          ...queued,
          stage: 'failed',
          error: String(error),
          message: 'Download could not start',
        });
      }
    } catch (error) {
      setError(String(error));
    } finally {
      setChoosing(false);
      choosingRef.current = false;
    }
  }

  async function chooseAnotherDestination() {
    if (
      busy ||
      resettingRef.current ||
      currentInspection.current ||
      choosingRef.current ||
      !desktopAvailable ||
      exampleMode ||
      !inspection ||
      inspectionStatus !== 'complete' ||
      !preferences.title.trim()
    )
      return;
    choosingRef.current = true;
    setChoosing(true);
    setError(null);
    try {
      const path = await chooseArchive(preferences.title);
      if (!path) return;
      setSavePath(path);
      retryOutput.current = path;
      retryTitle.current = preferences.title;
    } catch (error) {
      setError(String(error));
    } finally {
      choosingRef.current = false;
      setChoosing(false);
    }
  }

  async function cancel() {
    setCancelling(true);
    try {
      if (loading) {
        inspectionCancellation.current = true;
        await cancelInspection();
      } else if (currentJob.current) await cancelExport(currentJob.current);
    } catch (error) {
      setCancelling(false);
      if (!String(error).includes('no longer running')) setError(String(error));
    }
  }

  function toggle(index: number) {
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  }
  function updateTrack(index: number, track: Track) {
    if (
      inspection?.tracks[index]?.metadata.trackNumber !==
      track.metadata.trackNumber
    )
      setNumberingUndo(null);
    setInspection((value) =>
      value
        ? {
            ...value,
            tracks: value.tracks.map((existing, i) =>
              i === index ? track : existing,
            ),
          }
        : null,
    );
  }
  function updateAlbum(
    patch: Partial<
      Pick<ExportPreferences, 'albumTitle' | 'albumArtist' | 'year'>
    >,
  ) {
    if (busy) return;
    setPreferences((previous) =>
      updateAlbumPreferences(
        previous,
        patch,
        inspection?.title || 'Audio collection',
      ),
    );
  }
  function setMode(mode: CollectionMode) {
    if (busy) return;
    if (mode === 'album' && preferences.mode !== 'album') {
      setInspection((previous) =>
        previous ? withAlbumTrackNumbers(previous, mode) : null,
      );
    }
    setPreferences((previous) => ({
      ...previous,
      mode,
      albumTitle:
        mode === 'album'
          ? previous.albumTitle || inspection?.title || ''
          : previous.albumTitle,
      artworkMode:
        previous.artworkMode === 'none'
          ? 'none'
          : mode === 'album'
            ? 'shared'
            : 'perTrack',
    }));
  }
  function numberTracks() {
    if (busy || preferences.mode !== 'album') return;
    setNumberingUndo(
      inspection?.tracks.map((track) => track.metadata.trackNumber) ?? null,
    );
    setInspection((previous) =>
      previous
        ? { ...previous, tracks: numberAlbumTracks(previous.tracks) }
        : null,
    );
  }
  function undoNumbering() {
    if (busy || !numberingUndo) return;
    setInspection((previous) =>
      previous
        ? {
            ...previous,
            tracks: previous.tracks.map((track, index) => ({
              ...track,
              metadata: {
                ...track.metadata,
                trackNumber: numberingUndo[index] ?? null,
              },
            })),
          }
        : null,
    );
    setNumberingUndo(null);
  }
  async function reset() {
    if (
      busy ||
      resettingRef.current ||
      choosingRef.current ||
      currentInspection.current
    )
      return false;
    userStarted.current = true;
    // Keep one in-memory review until another source is successfully loaded.
    // A dismissed recovery record retains its cached files. Reusing the job ID
    // on retry creates a fresh recovery record and safely resumes that cache.
    const snapshot: ReviewSnapshot | null =
      inspection && inspectionStatus === 'complete'
        ? structuredClone({
            url,
            inspection,
            selected,
            preferences,
            inspectionTotal,
            progress,
            savePath,
            error,
            exampleMode,
            numberingUndo,
            currentJob: currentJob.current,
            retryOutput: retryOutput.current,
            retryTitle: retryTitle.current,
            auth: { browser, browserProfile },
          })
        : null;
    resettingRef.current = true;
    setResetting(true);
    try {
      if (currentJob.current && progress?.stage !== 'complete')
        await dismissRecovery(currentJob.current);
      if (snapshot) setPreviousReview(snapshot);
      setInspection(null);
      setNumberingUndo(null);
      setInspectionStatus('idle');
      setInspectionTotal(null);
      setUrl('');
      setSelected(new Set());
      setProgress(null);
      setError(null);
      setSavePath(null);
      setExampleMode(false);
      currentJob.current = null;
      retryOutput.current = null;
      retryTitle.current = null;
      return true;
    } catch (error) {
      setError(String(error));
      return false;
    } finally {
      resettingRef.current = false;
      setResetting(false);
    }
  }
  function undoReset() {
    if (
      !previousReview ||
      busy ||
      resettingRef.current ||
      choosingRef.current ||
      currentInspection.current
    )
      return false;
    userStarted.current = true;
    const snapshot = previousReview;
    setPreviousReview(null);
    setUrl(snapshot.url);
    setInspection(snapshot.inspection);
    setSelected(snapshot.selected);
    setPreferences(snapshot.preferences);
    setInspectionStatus('complete');
    setInspectionTotal(snapshot.inspectionTotal);
    setProgress(snapshot.progress);
    setSavePath(snapshot.savePath);
    setError(snapshot.error);
    setExampleMode(snapshot.exampleMode);
    setNumberingUndo(snapshot.numberingUndo);
    currentJob.current = snapshot.currentJob;
    retryOutput.current = snapshot.retryOutput;
    retryTitle.current = snapshot.retryTitle;
    browserSession.restore(snapshot.auth);
    return true;
  }
  function prepareAnotherCopy() {
    if (busy) return;
    setProgress(null);
    currentJob.current = null;
    retryOutput.current = null;
  }

  return {
    url,
    setUrl: changeUrl,
    browser,
    browserProfile,
    browserSession,
    inspection,
    selected,
    setSelected,
    preferences,
    setPreferences,
    loading,
    inspectionStatus,
    inspectionTotal,
    exporting,
    choosing,
    cancelling,
    busy,
    error,
    setError,
    progress,
    prepareAnotherCopy,
    eventsReady,
    ...downloaderState,
    dependencies,
    toolsReady,
    exampleMode,
    chosen,
    loadInspection,
    inspect,
    exportZip,
    chooseAnotherDestination,
    cancel,
    toggle,
    updateTrack,
    updateAlbum,
    setMode,
    numberTracks,
    numberingUndo,
    undoNumbering,
    destinationPath:
      retryTitle.current === preferences.title ? retryOutput.current : null,
    savePath,
    reset,
    undoReset,
    canUndoReset: !!previousReview && !busy,
    previousReview: previousReview
      ? {
          title:
            previousReview.preferences.title || previousReview.inspection.title,
          trackCount: previousReview.inspection.tracks.length,
        }
      : null,
  };
}
export type Workspace = ReturnType<typeof useWorkspace>;
