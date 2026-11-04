//! Versioned semantic themes, resolved only when the appearance changes.
use iced::{Color, Theme};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;

pub const MAX_THEME_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    #[default]
    Dark,
    Light,
    CatppuccinMocha,
}

impl Preset {
    pub const ALL: [Self; 3] = [Self::Dark, Self::Light, Self::CatppuccinMocha];
}

impl std::fmt::Display for Preset {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Dark => "Jacket Dark",
            Self::Light => "Jacket Light",
            Self::CatppuccinMocha => "Catppuccin Mocha",
        })
    }
}

/// A custom definition is kept with the preference, independent of its source file.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Selection {
    Preset { preset: Preset },
    Custom { name: String, source: String },
}

impl Default for Selection {
    fn default() -> Self {
        Self::Preset {
            preset: Preset::Dark,
        }
    }
}

impl Selection {
    pub fn resolve(&self, revision: u64) -> Result<ResolvedTheme, String> {
        match self {
            Self::Preset { preset } => Ok(ResolvedTheme::preset(*preset, revision)),
            Self::Custom { name, source } => {
                if name.trim().is_empty()
                    || name.chars().count() > 200
                    || name.chars().any(char::is_control)
                {
                    return Err("Give the custom theme a name of 1–200 characters.".into());
                }
                let mut resolved = ResolvedTheme::parse(source, revision)?;
                resolved.iced = crate::theme_iced::build(name, resolved.colors, resolved.derived);
                Ok(resolved)
            }
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Preset { preset } => preset.to_string(),
            Self::Custom { name, .. } => name.clone(),
        }
    }

    pub fn import(path: &Path) -> Result<Self, String> {
        let source = read_source(path)?;
        ResolvedTheme::parse(&source, 0).map_err(|error| format!("{}: {error}", path.display()))?;
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("Custom theme")
            .chars()
            .filter(|character| !character.is_control())
            .take(100)
            .collect::<String>();
        let selection = Self::Custom {
            name: if name.trim().is_empty() {
                "Custom theme".into()
            } else {
                name
            },
            source,
        };
        selection.resolve(0)?;
        Ok(selection)
    }
}

macro_rules! semantic_colors {
    ($($name:ident: $dark:expr => $light:expr => $mocha:expr),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Colors { $(pub $name: Color),+ }

        #[derive(Default, Deserialize)]
        #[serde(default, deny_unknown_fields)]
        struct Overrides { $($name: Option<String>),+ }

        impl Colors {
            pub fn preset(preset: Preset) -> Self {
                match preset {
                    Preset::Dark => Self { $($name: rgb($dark)),+ },
                    Preset::Light => Self { $($name: rgb($light)),+ },
                    Preset::CatppuccinMocha => Self { $($name: rgb($mocha)),+ },
                }
            }

            fn apply(&mut self, overrides: Overrides) -> Result<(), String> {
                $(if let Some(value) = overrides.$name {
                    self.$name = parse_color(&value).map_err(|error| format!("colors.{}: {error}", stringify!($name)))?;
                })+
                Ok(())
            }
        }
    };
}

// The shared schema lets the same theme file work in Jacket and Resonant.
// Track/audio roles are retained even though Jacket does not draw a timeline.
semantic_colors! {
    background: 0x1a1b1e => 0xf5f6f5 => 0x1e1e2e,
    chrome: 0x0d0e10 => 0xe7ebec => 0x11111b,
    panel: 0x141517 => 0xffffff => 0x181825,
    raised: 0x25262b => 0xedf0f1 => 0x313244,
    border: 0x2c2e33 => 0xcbd2d5 => 0x45475a,
    text: 0xc1c2c5 => 0x252b30 => 0xcdd6f4,
    text_muted: 0xa6a7ab => 0x56626a => 0xa6adc8,
    accent: 0xe29a6f => 0x914323 => 0xcba6f7,
    accent_strong: 0xe29a6f => 0x914323 => 0xcba6f7,
    accent_text: 0x20140d => 0xffffff => 0x1e1e2e,
    success: 0x51cf66 => 0x237a3c => 0xa6e3a1,
    warning: 0xffa94d => 0xa1560a => 0xf9e2af,
    danger: 0xff6b6b => 0xbf343b => 0xf38ba8,
    grid: 0x2c2e33 => 0xdce1e3 => 0x45475a,
    playhead: 0xff4444 => 0xc7363d => 0xf5e0dc,
    selection: 0xe29a6f => 0x914323 => 0x89b4fa,
    loop_region: 0x22b8cf => 0x08778b => 0x94e2d5,
    annotation: 0x22b8cf => 0x08778b => 0xb4befe,
    source: 0x22b8cf => 0x08778b => 0x94e2d5,
    drums: 0xf59f00 => 0x9a5800 => 0xfab387,
    bass: 0x339af0 => 0x226db3 => 0x89b4fa,
    other: 0x51cf66 => 0x2b783d => 0xa6e3a1,
    vocals: 0xf06595 => 0xb23868 => 0xf5c2e7,
    keys: 0xcc5de8 => 0x8444a5 => 0xb4befe,
    guitar: 0xfab005 => 0x8f6100 => 0xf2cdcd,
    meter_low: 0x51cf66 => 0x2b783d => 0xa6e3a1,
    meter_high: 0xfcc419 => 0xad7200 => 0xf9e2af,
    meter_clip: 0xff6b6b => 0xc7363d => 0xf38ba8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Definition {
    version: u32,
    base: Preset,
    #[serde(default)]
    colors: Overrides,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Derived {
    pub is_light: bool,
    pub hover: Color,
    pub pressed: Color,
    pub disabled: Color,
    pub selection_fill: Color,
    pub text_selection: Color,
    pub outline: Color,
    pub scrim: Color,
}

/// The application replaces this snapshot only after a theme resolves successfully.
#[derive(Clone, Debug)]
pub struct ResolvedTheme {
    pub colors: Colors,
    pub derived: Derived,
    pub iced: Theme,
    pub revision: u64,
}

impl ResolvedTheme {
    pub fn preset(preset: Preset, revision: u64) -> Self {
        Self::resolve(Colors::preset(preset), &preset.to_string(), revision)
    }

    pub fn parse(input: &str, revision: u64) -> Result<Self, String> {
        if input.len() > MAX_THEME_BYTES {
            return Err("Theme files must be 64 KiB or smaller.".into());
        }
        let definition: Definition = toml::from_str(input).map_err(|error| error.to_string())?;
        if definition.version != 1 {
            return Err(format!("version: expected 1, found {}", definition.version));
        }
        let follow_accent =
            definition.colors.accent.is_some() && definition.colors.accent_strong.is_none();
        let mut colors = Colors::preset(definition.base);
        colors.apply(definition.colors)?;
        if follow_accent {
            colors.accent_strong = colors.accent;
        }
        Ok(Self::resolve(colors, "Custom theme", revision))
    }

    fn resolve(colors: Colors, name: &str, revision: u64) -> Self {
        let is_light =
            contrast(colors.background, Color::BLACK) > contrast(colors.background, Color::WHITE);
        let derived = Derived {
            is_light,
            hover: blend(colors.text, colors.raised, 0.04),
            pressed: blend(colors.text, colors.raised, 0.08),
            disabled: blend(colors.text_muted, colors.panel, 0.65),
            selection_fill: blend(colors.selection, colors.panel, 0.10),
            text_selection: blend(colors.selection, colors.background, 0.30),
            outline: colors.text_muted,
            scrim: if is_light {
                Color {
                    a: 0.28,
                    ..colors.text
                }
            } else {
                Color::from_rgba(0.0, 0.0, 0.0, 0.65)
            },
        };
        Self {
            colors,
            derived,
            iced: crate::theme_iced::build(name, colors, derived),
            revision,
        }
    }
}

fn read_source(path: &Path) -> Result<String, String> {
    let metadata =
        std::fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err("Choose a regular TOML theme file.".into());
    }
    if metadata.len() > MAX_THEME_BYTES as u64 {
        return Err("Theme files must be 64 KiB or smaller.".into());
    }
    let file = std::fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !file
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err("Choose a regular TOML theme file.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_THEME_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_THEME_BYTES {
        return Err("Theme files must be 64 KiB or smaller.".into());
    }
    String::from_utf8(bytes).map_err(|_| "Theme files must use UTF-8 text.".into())
}

pub fn rgb(value: u32) -> Color {
    Color::from_rgb8((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

fn parse_color(value: &str) -> Result<Color, String> {
    let digits = value.strip_prefix('#').ok_or("expected #RRGGBB")?;
    if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{value:?}: expected #RRGGBB"));
    }
    u32::from_str_radix(digits, 16)
        .map(rgb)
        .map_err(|error| error.to_string())
}

pub fn blend(foreground: Color, background: Color, opacity: f32) -> Color {
    Color::from_rgb(
        foreground.r * opacity + background.r * (1.0 - opacity),
        foreground.g * opacity + background.g * (1.0 - opacity),
        foreground.b * opacity + background.b * (1.0 - opacity),
    )
}

pub fn contrast(first: Color, second: Color) -> f32 {
    fn luminance(color: Color) -> f32 {
        fn linear(channel: f32) -> f32 {
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
    }
    let (first, second) = (luminance(first), luminance(second));
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

#[cfg(test)]
mod tests;
