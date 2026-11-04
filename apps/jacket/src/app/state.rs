use crate::editor::{self, Editor};
use crate::settings::{
    Appearance, ArtworkChoice, BrowserChoice, FormatChoice, ModeChoice, OutputChoice,
};
use audio_engine::{AuthOptions, ExportPlan, ExportPreview, ExportResult, ProgressEvent};
use iced::window;
use jacket_core::{CompletionIdentity, Review, SourceIntent};
use jacket_desktop::artwork_assets::Asset;
use jacket_desktop::browser_sessions::Catalog;
use jacket_desktop::downloader_updates::{self, Status};
use jacket_desktop::recovery::Recovery;
use jacket_desktop::{Desktop, Job};
use std::fmt;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Workspace,
    Settings,
    Appearance,
    Options,
    SourceNotes,
    Preview,
    Editor,
    Crop,
    Replace,
    Discard,
    Close,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileChoice {
    pub id: String,
    pub label: String,
}
impl fmt::Display for ProfileChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

#[derive(Clone)]
pub struct Boot {
    pub(super) desktop: Desktop,
    pub(super) draft: Option<jacket_core::ReviewDraft>,
    pub(super) recovery: Option<Recovery>,
    pub(super) appearance: Appearance,
    pub(super) appearance_valid: bool,
    pub(super) downloader_preferences: downloader_updates::Preferences,
    pub(super) warning: Option<String>,
}

#[derive(Clone)]
pub enum Message {
    Tick,
    NativeReady,
    DisplayScale(f32),
    OwnedCoverRaster(String, f32, Result<crate::artwork::Raster, String>),
    Quit,
    RetryStartup,
    Booted(Result<Box<Boot>, String>),
    Url(String),
    Pasted(String),
    Intent(SourceIntent),
    Find,
    ConfirmFind,
    Example,
    TrackScrolled(f32),
    Toggle(usize, bool),
    SelectAll(bool),
    OpenEditor(editor::Kind),
    Editor(editor::Action),
    Format(FormatChoice),
    Artwork(ArtworkChoice),
    Output(OutputChoice),
    Mode(ModeChoice),
    OpenPanel(Panel),
    ClosePanel,
    Preview,
    Save,
    Destination(Option<PathBuf>),
    Cancel,
    Saved(CompletionIdentity, Result<bool, String>),
    DismissError,
    Reset,
    ConfirmReset,
    Resume,
    DismissRecovery,
    ExportSaved(
        Box<ExportPlan>,
        PathBuf,
        CompletionIdentity,
        Result<bool, String>,
    ),
    ReviewDiscarded(Result<bool, String>),
    UndoBulk,
    NumberTracks,
    UndoNumbering,
    RecoveryDismissed(Result<(), String>),
    CloseRequested(window::Id),
    ConfirmClose,
    CloseReady(Result<(), String>),
    Browser(BrowserChoice),
    Profiles(BrowserChoice, Catalog),
    Profile(ProfileChoice),
    UseSession,
    PublicAccess,
    TestAccess,
    ThemePreset(crate::theme::Preset),
    ImportTheme,
    ThemeFile(Option<PathBuf>),
    ThemeImported(Result<crate::theme::Selection, String>),
    PasteLink,
    PrimaryAction,
    ThemeShortcut(crate::theme::Preset),
    FocusNext,
    FocusPrevious,
    DefaultFormat(FormatChoice),
    AppearanceSaved(Result<(), String>),
    CheckDownloader,
    InstallDownloader,
    RestoreDownloader,
    AutoUpdate(bool),
    PickDownloader,
    DownloaderPath(Option<PathBuf>),
    SourceCover(String, Result<crate::artwork::Raster, String>),
    PreviewCover(String, Result<crate::artwork::Raster, String>),
    Dropped(PathBuf),
    ImportCover,
    CoverFile(Option<PathBuf>),
    CoverImported(Result<(Asset, PathBuf), String>),
    RemoteCoverImported(CompletionIdentity, Result<(Asset, PathBuf), String>),
    RetryArtwork,
    CropPointer(iced::Point),
    CropDown,
    CropUp,
    CropCover,
    CropX(String),
    CropY(String),
    CropSize(String),
    ApplyCrop,
    OpenOutput,
    OpenPlaylist,
    Opened(Result<(), String>),
    Screenshot(window::Screenshot),
    ScreenshotSaved(Result<(), String>),
    FocusSource,
    Escape,
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Message")
            .field("kind", &std::mem::discriminant(self))
            .finish()
    }
}

pub(super) struct SuspendedReview {
    pub(super) review: Option<Review>,
    pub(super) url: String,
    pub(super) cover: Option<(Asset, PathBuf)>,
    pub(super) source_cover: Option<crate::artwork::Raster>,
    pub(super) source_cover_error: Option<String>,
    pub(super) track_scroll: f32,
    pub(super) result: Option<ExportResult>,
    pub(super) appearance: Appearance,
}

pub struct App {
    pub desktop: Option<Desktop>,
    pub review: Option<Review>,
    pub job: Option<Job>,
    pub url: String,
    pub intent: SourceIntent,
    pub panel: Panel,
    pub editor: Option<Editor>,
    pub preview: Option<ExportPreview>,
    pub progress: Option<ProgressEvent>,
    pub result: Option<ExportResult>,
    pub recovery: Option<Recovery>,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub appearance: Appearance,
    pub theme: crate::theme::ResolvedTheme,
    pub display_scale: f32,
    pub(super) owned_rasters: std::collections::BTreeMap<String, crate::artwork::Raster>,
    pub(super) raster_loading: std::collections::BTreeSet<(String, u32)>,
    pub(super) raster_refresh_pending: bool,
    pub example: bool,
    pub loading: bool,
    pub dialog_open: bool,
    pub save_pending: usize,
    pub save_failed: bool,
    pub browser: BrowserChoice,
    pub profiles: Vec<ProfileChoice>,
    pub profile: Option<ProfileChoice>,
    pub profile_notice: Option<String>,
    pub auth: AuthOptions,
    pub downloader: Option<Status>,
    pub downloader_preferences: downloader_updates::Preferences,
    pub cover: Option<(Asset, PathBuf)>,
    pub source_cover: Option<crate::artwork::Raster>,
    pub preview_cover: Option<crate::artwork::Raster>,
    pub source_cover_error: Option<String>,
    pub preview_cover_error: Option<String>,
    pub(super) crop_source: Option<(Asset, PathBuf)>,
    pub track_scroll: f32,
    pub crop_x: String,
    pub crop_y: String,
    pub crop_size: String,
    pub(super) pending_plan: Option<ExportPlan>,
    pub(super) close_window: Option<window::Id>,
    pub(super) close_after_job: bool,
    pub(super) screenshot: Option<PathBuf>,
    pub(super) started: Instant,
    pub(super) example_previous: Option<SuspendedReview>,
    pub(super) resumed_previous: Option<(SuspendedReview, Option<CompletionIdentity>)>,
    pub(super) crop_pointer: iced::Point,
    pub(super) crop_dragging: bool,
    pub(super) appearance_saving: bool,
    pub(super) appearance_dirty: bool,
    pub(super) appearance_valid: bool,
    pub(super) _example_cover_file: Option<tempfile::NamedTempFile>,
}
