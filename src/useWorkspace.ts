import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  archiveDestination,
  cancelExport,
  cancelInspection,
  chooseArchive,
  chooseFolder,
  dependencyStatus,
  desktopAvailable,
  discardReviewDraft,
  dismissRecovery,
  inspectSource,
  loadReviewDraft,
  pendingExport,
  saveReviewDraft,
  startExport,
} from './desktop';
import {
  type CollectionMode,
  type ExportPlan,
  type Inspection,
  type InspectionEvent,
  type JobEvent,
  type PendingExport,
  type ReviewCompletionIdentity,
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
import { makeReviewDraft, type ReviewDraft } from './reviewDraft';
import {
  applyBulkEdit,
  type BulkEditUndo,
  type BulkTrackEdits,
  undoBulkEdit,
} from './trackEdits';
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
  sharedArtworkAsset: null,
  outputKind: 'zip',
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
  draftSession: string;
  draftRevision: number;
  completedContent: string | null;
  recoveredReview: RecoveredReview | null;
  savedReview: ReviewDraft | null;
}

interface RecoveredReview {
  snapshot: string;
  identity: ReviewCompletionIdentity | null;
  unrelatedDraft: boolean;
}

interface DraftWrite {
  identity: ReviewCompletionIdentity;
  snapshot: string;
  content: string;
  status: 'saving' | 'saved' | 'failed';
  promise: Promise<boolean>;
}

function reviewSignature(
  inspection: Inspection,
  selected: ReadonlySet<number>,
  preferences: ExportPreferences,
  includeOutput = false,
): string {
  const { format, outputKind, ...contentPreferences } = preferences;
  return JSON.stringify({
    inspection,
    selected: [...selected].sort((a, b) => a - b),
    preferences: includeOutput
      ? { ...contentPreferences, format, outputKind }
      : contentPreferences,
  });
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
  const draftSession = useRef<string>(crypto.randomUUID());
  const draftRevision = useRef(0);
  const [draftReady, setDraftReady] = useState(!desktopAvailable);
  const [draftStatus, setDraftStatus] = useState<
    'saving' | 'saved' | 'failed' | null
  >(null);
  const [bulkUndo, setBulkUndo] = useState<BulkEditUndo | null>(null);
  const [savedReview, setSavedReview] = useState<ReviewDraft | null>(null);
  // Finished albums remain available in memory. Output conveniences do not
  // turn a finished album into unfinished work; committed content changes do.
  const completedContent = useRef<string | null>(null);
  const currentContent = useRef<string | null>(null);
  const currentSnapshot = useRef<string | null>(null);
  const recoveredReview = useRef<RecoveredReview | null>(null);
  const latestDraftWrite = useRef<DraftWrite | null>(null);
  const activeExportReview = useRef<{
    jobId: string;
    content: string;
    identity: ReviewCompletionIdentity | null;
  } | null>(null);
  currentContent.current = useMemo(
    () =>
      inspection ? reviewSignature(inspection, selected, preferences) : null,
    [inspection, selected, preferences],
  );
  currentSnapshot.current = useMemo(
    () =>
      inspection
        ? reviewSignature(inspection, selected, preferences, true)
        : null,
    [inspection, selected, preferences],
  );

  const writeReviewDraft = useCallback(
    (
      inspection: Inspection,
      selected: ReadonlySet<number>,
      preferences: ExportPreferences,
    ): DraftWrite => {
      const snapshot = reviewSignature(inspection, selected, preferences, true);
      const previous = latestDraftWrite.current;
      if (
        previous?.identity.sessionId === draftSession.current &&
        previous.identity.revision === draftRevision.current &&
        previous.snapshot === snapshot &&
        previous.status !== 'failed'
      )
        return previous;
      const identity = {
        sessionId: draftSession.current,
        revision: ++draftRevision.current,
      };
      const write: DraftWrite = {
        identity,
        snapshot,
        content: reviewSignature(inspection, selected, preferences),
        status: 'saving',
        promise: Promise.resolve(false),
      };
      latestDraftWrite.current = write;
      setDraftStatus('saving');
      const canReport = () =>
        draftSession.current === identity.sessionId &&
        draftRevision.current === identity.revision &&
        currentContent.current === write.content &&
        completedContent.current === null &&
        !resettingRef.current;
      write.promise = saveReviewDraft(
        makeReviewDraft(
          identity.sessionId,
          identity.revision,
          inspection,
          selected,
          preferences,
        ),
      ).then(
        (saved) => {
          write.status = saved ? 'saved' : 'failed';
          if (canReport()) setDraftStatus(write.status);
          return saved;
        },
        (error) => {
          write.status = 'failed';
          if (canReport()) setDraftStatus('failed');
          throw error;
        },
      );
      return write;
    },
    [],
  );

  const installReviewDraft = useCallback((draft: ReviewDraft) => {
    draftSession.current = draft.sessionId;
    draftRevision.current = draft.revision;
    completedContent.current = null;
    latestDraftWrite.current = null;
    recoveredReview.current = null;
    setUrl(draft.url);
    setInspection(draft.inspection);
    setInspectionStatus('complete');
    setInspectionTotal(draft.inspection.tracks.length);
    setSelected(new Set(draft.selected));
    setPreferences({ ...defaults, ...draft.preferences });
    setBulkUndo(null);
    setNumberingUndo(null);
  }, []);

  const restorePendingExport = useCallback(
    (value: PendingExport) => {
      const plan = value.plan;
      const inspection: Inspection = {
        title: plan.title,
        sourceUrl: plan.sourceUrl,
        suggestedMode: plan.mode,
        artworkUrl:
          plan.sharedArtworkUrl ||
          plan.tracks.find((track) => track.artworkUrl)?.artworkUrl ||
          null,
        tracks: plan.tracks,
        warnings: [],
      };
      const selected = new Set(plan.tracks.map((_, index) => index));
      const preferences = recoveryPreferences(plan);
      recoveredReview.current = {
        snapshot: reviewSignature(inspection, selected, preferences, true),
        identity: value.reviewIdentity ?? null,
        unrelatedDraft: false,
      };
      draftSession.current =
        value.reviewIdentity?.sessionId ?? crypto.randomUUID();
      draftRevision.current = value.reviewIdentity?.revision ?? 0;
      latestDraftWrite.current = null;
      completedContent.current = null;
      setInspection(inspection);
      setSelected(selected);
      setPreferences(preferences);
      browserSession.restore(plan.auth);
      setUrl(plan.sourceUrl);
      setInspectionStatus('complete');
      setInspectionTotal(plan.tracks.length);
      currentJob.current = value.jobId;
      retryOutput.current = value.outputPath;
      retryTitle.current = `${plan.title}\0${plan.outputKind ?? 'zip'}`;
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
      retryTitle.current === `${preferences.title}\0${preferences.outputKind}`
    ) {
      setSavePath(retryOutput.current);
      return;
    }
    setSavePath(null);
    if (preferences.outputKind === 'folder') return;
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
  }, [
    preferences.title,
    preferences.outputKind,
    exampleMode,
    inspectionStatus,
    progress?.stage,
  ]);

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
      .then(async (value) => {
        if (disposed || userStarted.current) return;
        if (value) restorePendingExport(value);
        const draft = await loadReviewDraft();
        if (disposed || userStarted.current || !draft) return;
        if (
          value &&
          (draft.inspection.sourceUrl !== value.plan.sourceUrl ||
            (value.reviewIdentity &&
              draft.sessionId !== value.reviewIdentity.sessionId))
        ) {
          if (recoveredReview.current)
            recoveredReview.current.unrelatedDraft = true;
          setSavedReview(draft);
          return;
        }
        userStarted.current = true;
        installReviewDraft(draft);
        setDraftStatus('saved');
      })
      .catch((error) => {
        if (!disposed && !userStarted.current) {
          setDraftStatus('failed');
          setError(String(error));
        }
      })
      .finally(() => {
        if (!disposed) setDraftReady(true);
      });
    let unsubscribe: (() => void) | undefined;
    listen<JobEvent>('export-progress', ({ payload }) => {
      if (disposed || payload.jobId !== currentJob.current) return;
      if (payload.stage === 'complete') {
        const submitted = activeExportReview.current;
        if (
          submitted?.jobId === payload.jobId &&
          submitted.content === currentContent.current
        ) {
          completedContent.current = submitted.content;
          recoveredReview.current = null;
          setDraftStatus(null);
        }
      }
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
  }, [restorePendingExport, installReviewDraft]);

  useEffect(() => {
    if (
      !desktopAvailable ||
      !draftReady ||
      exampleMode ||
      !inspection ||
      inspectionStatus !== 'complete' ||
      resettingRef.current
    )
      return;
    const content = reviewSignature(inspection, selected, preferences);
    if (completedContent.current === content) return;
    const snapshot = reviewSignature(inspection, selected, preferences, true);
    if (recoveredReview.current?.snapshot === snapshot) return;
    if (completedContent.current || recoveredReview.current) {
      // A new committed album edit gets a new generation. The native store
      // fences delayed writes belonging to the finished snapshot.
      draftSession.current = crypto.randomUUID();
      draftRevision.current = 0;
      latestDraftWrite.current = null;
      completedContent.current = null;
      recoveredReview.current = null;
      setSavedReview(null);
    }
    void writeReviewDraft(inspection, selected, preferences).promise.catch(
      () => {},
    );
  }, [
    draftReady,
    exampleMode,
    inspection,
    inspectionStatus,
    selected,
    preferences,
    writeReviewDraft,
  ]);

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
    setBulkUndo(null);
    setSavedReview(null);
    draftSession.current = crypto.randomUUID();
    draftRevision.current = 0;
    latestDraftWrite.current = null;
    completedContent.current = null;
    recoveredReview.current = null;
    activeExportReview.current = null;
    setDraftStatus(null);
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
    setBulkUndo(null);
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
      !draftReady ||
      !desktopAvailable ||
      exampleMode
    )
      return;
    const content = reviewSignature(inspection, selected, preferences);
    const snapshot = reviewSignature(inspection, selected, preferences, true);
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
        retryTitle.current === `${preferences.title}\0${preferences.outputKind}`
          ? retryOutput.current
          : await (preferences.outputKind === 'folder'
              ? chooseFolder(preferences.title)
              : chooseArchive(preferences.title));
      if (!path) return;
      if (currentSnapshot.current !== snapshot)
        throw new Error(
          'Your review changed while choosing a destination. Start the download again.',
        );
      setSavePath(path);
      retryOutput.current = path;
      retryTitle.current = `${preferences.title}\0${preferences.outputKind}`;
      let reviewIdentity: ReviewCompletionIdentity | null = null;
      const recovered = recoveredReview.current;
      if (completedContent.current !== content) {
        if (recovered?.unrelatedDraft && recovered.snapshot === snapshot) {
          // Recovery already durably contains this exact plan. Re-saving it
          // would replace a different unfinished review kept by the user.
          reviewIdentity = recovered.identity;
        } else {
          const write = writeReviewDraft(inspection, selected, preferences);
          if (!(await write.promise))
            throw new Error(
              'Your review could not be saved. Keep Jacket open and try the download again.',
            );
          if (
            currentSnapshot.current !== snapshot ||
            draftSession.current !== write.identity.sessionId ||
            draftRevision.current !== write.identity.revision
          )
            throw new Error(
              'Your review changed before the download started. Start the download again.',
            );
          reviewIdentity = write.identity;
        }
      }
      const id =
        currentJob.current &&
        ['failed', 'cancelled', 'interrupted'].includes(progress?.stage ?? '')
          ? currentJob.current
          : crypto.randomUUID();
      currentJob.current = id;
      activeExportReview.current = {
        jobId: id,
        content,
        identity: reviewIdentity,
      };
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
            sharedArtworkAsset: preferences.sharedArtworkAsset,
            outputKind: preferences.outputKind,
            auth: {
              browser,
              browserProfile,
            },
            tracks: chosen,
          },
          path,
          id,
          reviewIdentity,
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
      const path = await (preferences.outputKind === 'folder'
        ? chooseFolder(preferences.title)
        : chooseArchive(preferences.title));
      if (!path) return;
      setSavePath(path);
      retryOutput.current = path;
      retryTitle.current = `${preferences.title}\0${preferences.outputKind}`;
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
    userStarted.current = true;
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  }
  function updateTrack(index: number, track: Track) {
    userStarted.current = true;
    setBulkUndo(null);
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
    userStarted.current = true;
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
    userStarted.current = true;
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
    userStarted.current = true;
    setBulkUndo(null);
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
    userStarted.current = true;
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
            draftSession: draftSession.current,
            draftRevision: draftRevision.current,
            completedContent: completedContent.current,
            recoveredReview: recoveredReview.current,
            savedReview,
          })
        : null;
    resettingRef.current = true;
    setResetting(true);
    try {
      if (currentJob.current && progress?.stage !== 'complete')
        await dismissRecovery(currentJob.current);
      if (snapshot) setPreviousReview(snapshot);
      setInspection(null);
      currentContent.current = null;
      currentSnapshot.current = null;
      setDraftStatus(null);
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
    draftSession.current = snapshot.draftSession;
    draftRevision.current = Math.max(
      draftRevision.current,
      snapshot.draftRevision,
    );
    completedContent.current = snapshot.completedContent;
    recoveredReview.current = snapshot.recoveredReview;
    latestDraftWrite.current = null;
    setSavedReview(snapshot.savedReview);
    setDraftStatus(null);
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

  function applyBulk(edits: BulkTrackEdits) {
    if (busy || !inspection || inspectionStatus !== 'complete') return;
    const result = applyBulkEdit(inspection.tracks, selected, edits);
    if (!result.changedCount) return;
    userStarted.current = true;
    setInspection({ ...inspection, tracks: result.tracks });
    setNumberingUndo(null);
    setBulkUndo(result.undo);
  }

  function undoBulk() {
    if (busy || !inspection || !bulkUndo || inspectionStatus !== 'complete')
      return;
    userStarted.current = true;
    setInspection({
      ...inspection,
      tracks: undoBulkEdit(inspection.tracks, bulkUndo),
    });
    setBulkUndo(null);
  }

  async function discardDraft() {
    if (busy) return;
    if (savedReview) {
      await continueSavedReview();
      return;
    }
    resettingRef.current = true;
    setResetting(true);
    try {
      // Keep the newer committed review if export recovery cannot be dismissed.
      if (currentJob.current && progress?.stage !== 'complete')
        await dismissRecovery(currentJob.current);
      const discarded = await discardReviewDraft(
        draftSession.current,
        ++draftRevision.current,
      );
      if (!discarded)
        throw new Error(
          'The saved review changed. Reopen Jacket before discarding it.',
        );
      currentJob.current = null;
      resettingRef.current = false;
      setResetting(false);
      if (await reset()) {
        setPreviousReview(null);
        setDraftStatus(null);
        setBulkUndo(null);
      }
    } catch (error) {
      setError(String(error));
    } finally {
      resettingRef.current = false;
      setResetting(false);
    }
  }

  async function continueSavedReview() {
    if (busy || !savedReview) return;
    resettingRef.current = true;
    setResetting(true);
    try {
      if (currentJob.current && progress?.stage !== 'complete')
        await dismissRecovery(currentJob.current);
      userStarted.current = true;
      draftSession.current = savedReview.sessionId;
      draftRevision.current = Math.max(
        draftRevision.current,
        savedReview.revision,
      );
      installReviewDraft(savedReview);
      currentJob.current = null;
      retryOutput.current = null;
      retryTitle.current = null;
      setProgress(null);
      setSavePath(null);
      setError(null);
      setSavedReview(null);
      setPreviousReview(null);
      setExampleMode(false);
      browserSession.restore({ browser: null, browserProfile: null });
    } catch (error) {
      setError(String(error));
    } finally {
      resettingRef.current = false;
      setResetting(false);
    }
  }

  return {
    url,
    setUrl: changeUrl,
    browser,
    browserProfile,
    browserSession,
    inspection,
    selected,
    setSelected: (value: Parameters<typeof setSelected>[0]) => {
      userStarted.current = true;
      setSelected(value);
    },
    preferences,
    setPreferences: (value: Parameters<typeof setPreferences>[0]) => {
      userStarted.current = true;
      setPreferences(value);
    },
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
    draftReady,
    ...downloaderState,
    dependencies,
    toolsReady,
    exampleMode,
    chosen,
    exportPlan: inspection
      ? ({
          title: preferences.title,
          sourceUrl: inspection.sourceUrl,
          mode: preferences.mode,
          format: preferences.format,
          artworkMode: preferences.artworkMode,
          sharedArtworkUrl: preferences.sharedArtwork,
          sharedArtworkAsset: preferences.sharedArtworkAsset,
          outputKind: preferences.outputKind,
          auth: { browser, browserProfile },
          tracks: chosen,
        } satisfies ExportPlan)
      : null,
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
    applyBulk,
    undoBulk,
    bulkUndo,
    draftStatus,
    discardDraft,
    savedReview: savedReview
      ? {
          title: savedReview.preferences.title,
          trackCount: savedReview.selected.length,
        }
      : null,
    continueSavedReview,
    destinationPath:
      retryTitle.current === `${preferences.title}\0${preferences.outputKind}`
        ? retryOutput.current
        : null,
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
