use audio_engine::{ArtworkMode, AudioFormat, Browser, CollectionMode, OutputKind};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

macro_rules! choices {
    ($name:ident { $($variant:ident => $label:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name { $($variant),+ }
        impl $name { pub const ALL: &'static [Self] = &[$(Self::$variant),+]; }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(match self { $(Self::$variant => $label),+ })
            }
        }
    };
}
choices!(FormatChoice { Compatible => "M4A / MP3", Preserve => "Keep source codec", Mp3 => "MP3" });
choices!(ArtworkChoice { Shared => "Shared cover", PerTrack => "Per-track artwork", None => "No artwork" });
choices!(OutputChoice { File => "Audio file", Folder => "Music folder", Zip => "ZIP archive" });
choices!(ModeChoice { Album => "Album · shared details", Collection => "Playlist · individual details" });
choices!(BrowserChoice { Public => "Public access", Chrome => "Google Chrome", Firefox => "Firefox", Edge => "Microsoft Edge", Safari => "Safari" });
choices!(EditAction { Keep => "Keep", Set => "Set", Clear => "Clear" });

impl From<FormatChoice> for AudioFormat {
    fn from(value: FormatChoice) -> Self {
        match value {
            FormatChoice::Compatible => Self::Compatible,
            FormatChoice::Preserve => Self::Preserve,
            FormatChoice::Mp3 => Self::Mp3,
        }
    }
}
impl From<AudioFormat> for FormatChoice {
    fn from(value: AudioFormat) -> Self {
        match value {
            AudioFormat::Compatible => Self::Compatible,
            AudioFormat::Preserve => Self::Preserve,
            AudioFormat::Mp3 => Self::Mp3,
        }
    }
}
impl From<ArtworkChoice> for ArtworkMode {
    fn from(value: ArtworkChoice) -> Self {
        match value {
            ArtworkChoice::Shared => Self::Shared,
            ArtworkChoice::PerTrack => Self::PerTrack,
            ArtworkChoice::None => Self::None,
        }
    }
}
impl From<ArtworkMode> for ArtworkChoice {
    fn from(value: ArtworkMode) -> Self {
        match value {
            ArtworkMode::Shared => Self::Shared,
            ArtworkMode::PerTrack => Self::PerTrack,
            ArtworkMode::None => Self::None,
        }
    }
}
impl From<OutputChoice> for OutputKind {
    fn from(value: OutputChoice) -> Self {
        match value {
            OutputChoice::File => Self::File,
            OutputChoice::Folder => Self::Folder,
            OutputChoice::Zip => Self::Zip,
        }
    }
}
impl From<OutputKind> for OutputChoice {
    fn from(value: OutputKind) -> Self {
        match value {
            OutputKind::File => Self::File,
            OutputKind::Folder => Self::Folder,
            OutputKind::Zip => Self::Zip,
        }
    }
}
impl From<ModeChoice> for CollectionMode {
    fn from(value: ModeChoice) -> Self {
        match value {
            ModeChoice::Album => Self::Album,
            ModeChoice::Collection => Self::Collection,
        }
    }
}
impl From<CollectionMode> for ModeChoice {
    fn from(value: CollectionMode) -> Self {
        match value {
            CollectionMode::Album => Self::Album,
            CollectionMode::Collection => Self::Collection,
        }
    }
}
impl BrowserChoice {
    pub fn browser(self) -> Option<Browser> {
        match self {
            Self::Public => None,
            Self::Chrome => Some(Browser::Chrome),
            Self::Firefox => Some(Browser::Firefox),
            Self::Edge => Some(Browser::Edge),
            Self::Safari => Some(Browser::Safari),
        }
    }
}

/// Application appearance is independent of review and export data.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Appearance {
    pub theme: crate::theme::Selection,
    pub default_format: FormatPreference,
}

impl<'de> Deserialize<'de> for Appearance {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Default, Deserialize)]
        #[serde(default, deny_unknown_fields)]
        struct Stored {
            theme: Option<crate::theme::Selection>,
            dark: Option<bool>,
            default_format: FormatPreference,
        }
        let stored = Stored::deserialize(deserializer)?;
        let theme = stored.theme.unwrap_or(crate::theme::Selection::Preset {
            preset: match stored.dark {
                Some(false) => crate::theme::Preset::Light,
                _ => crate::theme::Preset::Dark,
            },
        });
        Ok(Self {
            theme,
            default_format: stored.default_format,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatPreference {
    #[default]
    Compatible,
    Preserve,
    Mp3,
}
impl From<FormatPreference> for AudioFormat {
    fn from(value: FormatPreference) -> Self {
        match value {
            FormatPreference::Compatible => Self::Compatible,
            FormatPreference::Preserve => Self::Preserve,
            FormatPreference::Mp3 => Self::Mp3,
        }
    }
}
impl From<FormatChoice> for FormatPreference {
    fn from(value: FormatChoice) -> Self {
        match value {
            FormatChoice::Compatible => Self::Compatible,
            FormatChoice::Preserve => Self::Preserve,
            FormatChoice::Mp3 => Self::Mp3,
        }
    }
}
