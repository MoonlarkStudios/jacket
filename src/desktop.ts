import { invoke, isTauri } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import type {
  ArtworkAsset,
  ArtworkCrop,
  Browser,
  DownloaderChannel,
  DownloaderPreferences,
  DownloaderStatus,
  ExportPlan,
  ExportPreview,
  Inspection,
  PendingExport,
  ReviewCompletionIdentity,
} from './domain';
import {
  archiveDirectory,
  archiveFilename,
  readPreference,
  writePreference,
} from './preferences';
import type { ReviewDraft } from './reviewDraft';

export const desktopAvailable = isTauri();

export function inspectSource(
  url: string,
  browser: Browser | null,
  inspectionId: string,
  browserProfile: string | null = null,
) {
  return invoke<Inspection>('inspect_source', {
    url,
    auth: { browser, browserProfile },
    inspectionId,
  });
}

export interface BrowserProfile {
  id: string;
  label: string;
  detail: string;
}

export interface BrowserProfiles {
  profiles: BrowserProfile[];
  notice: string | null;
}

export interface BrowserAccessCheck {
  accessible: boolean;
  message: string;
  guidance: string;
}

export function browserProfiles(browser: Browser) {
  return invoke<BrowserProfiles>('browser_profiles', { browser });
}

export function testBrowserAccess(
  browser: Browser,
  browserProfile: string,
  url: string,
) {
  return invoke<BrowserAccessCheck>('test_browser_access', {
    auth: { browser, browserProfile },
    url,
  });
}

export interface SessionStatus {
  supported: boolean;
  sessionId: string | null;
  verified: boolean;
  signingIn: boolean;
  message: string;
}

export function sessionStatus() {
  return invoke<SessionStatus>('session_status');
}

export function sessionAction(
  action: 'connect' | 'save' | 'verify' | 'disconnect',
  url: string | null = null,
) {
  return invoke<SessionStatus>('session_action', { action, url });
}

export async function chooseArchive(title: string) {
  const filename = archiveFilename(title);
  const directory = readPreference('save-directory') || '';
  const defaultPath = await archiveDestination(title).catch(
    () => `${directory}${filename}`,
  );
  const path = await save({
    title: 'Save audio archive',
    defaultPath,
    filters: [{ name: 'ZIP archive', extensions: ['zip'] }],
  });
  if (path) writePreference('save-directory', archiveDirectory(path));
  return path;
}

export function archiveDestination(title: string) {
  return invoke<string>('archive_destination', {
    directory: readPreference('save-directory'),
    filename: archiveFilename(title),
  });
}

export function startExport(
  plan: ExportPlan,
  outputPath: string,
  jobId: string,
  reviewIdentity: ReviewCompletionIdentity | null = null,
) {
  return invoke<void>('start_export', {
    plan,
    outputPath,
    jobId,
    reviewIdentity,
  });
}

export function cancelExport(jobId: string) {
  return invoke<void>('cancel_export', { jobId });
}

export function cancelInspection() {
  return cancelExport('inspection');
}

export function dependencyStatus() {
  return invoke<Array<{ name: string; available: boolean; bundled?: boolean }>>(
    'dependency_status',
  );
}

export function revealArchive(outputPath: string) {
  return invoke<void>('reveal_archive', { outputPath });
}

export async function chooseFolder(title: string) {
  const parent = await open({
    title: 'Choose a folder for your album',
    directory: true,
    multiple: false,
    defaultPath: readPreference('save-directory') || undefined,
  });
  if (typeof parent !== 'string') return null;
  const separator = parent.includes('\\') ? '\\' : '/';
  writePreference(
    'save-directory',
    `${parent.replace(/[\\/]$/, '')}${separator}`,
  );
  return invoke<string>('folder_destination', { directory: parent, title });
}

export function previewExport(plan: ExportPlan) {
  return invoke<ExportPreview>('preview_export', { plan });
}

export function openPlaylist(outputPath: string) {
  return invoke<void>('open_export_playlist', { outputPath });
}

export async function chooseArtwork() {
  const path = await open({
    title: 'Choose cover artwork',
    multiple: false,
    directory: false,
    filters: [{ name: 'Cover image', extensions: ['jpg', 'jpeg', 'png'] }],
  });
  return typeof path === 'string' ? importArtwork(path) : null;
}

export function importArtwork(path: string) {
  return invoke<ArtworkAsset>('import_artwork', { path, crop: null });
}

export function cropArtwork(id: string, crop: ArtworkCrop) {
  return invoke<ArtworkAsset>('crop_artwork', { id, crop });
}

export function artworkAsset(id: string) {
  return invoke<ArtworkAsset>('artwork_asset', { id });
}

export function loadReviewDraft() {
  return invoke<ReviewDraft | null>('review_draft_load');
}

export function saveReviewDraft(draft: ReviewDraft) {
  return invoke<boolean>('review_draft_save', { draft });
}

export function discardReviewDraft(sessionId: string, revision: number) {
  return invoke<boolean>('review_draft_discard', { sessionId, revision });
}

export function pendingExport() {
  return invoke<PendingExport | null>('pending_export');
}

export function dismissRecovery(jobId: string) {
  return invoke<void>('dismiss_recovery', { jobId });
}

export function checkDownloaderUpdate(
  force = false,
  channel: DownloaderChannel | null = null,
) {
  return invoke<DownloaderStatus>('check_downloader_update', {
    force,
    channel,
  });
}
export function updateDownloader(channel: DownloaderChannel) {
  return invoke<DownloaderStatus>('update_downloader', { channel });
}
export function restoreDownloader() {
  return invoke<DownloaderStatus>('restore_downloader');
}

export function configureDownloader(preferences: DownloaderPreferences) {
  return invoke<DownloaderStatus>('configure_downloader', { preferences });
}
export function cancelDownloaderUpdate() {
  return invoke<void>('cancel_downloader_update');
}
export async function chooseDownloader(defaultPath?: string) {
  const result = await open({
    title: 'Choose yt-dlp executable',
    defaultPath,
    multiple: false,
    directory: false,
  });
  return typeof result === 'string'
    ? invoke<string>('downloader_selection_path', {
        path: result,
        preferredPath: defaultPath ?? null,
      })
    : null;
}
