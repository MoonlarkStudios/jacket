//! Small, stroke-based controls; authored as vectors and recolored by the theme.
use crate::app::Message;
use iced::widget::svg;
use iced::{Color, Element};

#[derive(Clone, Copy)]
pub enum Icon {
    Settings,
    Palette,
    Link,
    ArrowRight,
    Music,
    Folder,
    File,
    Archive,
    Crop,
    Close,
    Upload,
    Check,
    Edit,
}

pub fn view(icon: Icon, color: Color, size: f32) -> Element<'static, Message> {
    let shape = match icon {
        Icon::Settings => {
            r#"<path d="M12 8v.01M6 4h12M6 12h12M6 20h12"/><circle cx="8" cy="4" r="2"/><circle cx="16" cy="12" r="2"/><circle cx="10" cy="20" r="2"/>"#
        }
        Icon::Palette => {
            r#"<path d="M12 3a9 9 0 1 0 0 18h1a2 2 0 0 0 1.5-3.3 1.5 1.5 0 0 1 1-2.7H18a4 4 0 0 0 4-4c0-4.4-4.5-8-10-8Z"/><circle cx="7" cy="11" r=".8"/><circle cx="10" cy="7" r=".8"/><circle cx="15" cy="7" r=".8"/>"#
        }
        Icon::Link => {
            r#"<path d="m10 13 4-4m-6 6-2 2a4 4 0 0 1-6-6l4-4a4 4 0 0 1 6 0m4 2 2-2a4 4 0 0 1 6 6l-4 4a4 4 0 0 1-6 0" transform="translate(2 0) scale(.9)"/>"#
        }
        Icon::ArrowRight => r#"<path d="M4 12h16m-6-6 6 6-6 6"/>"#,
        Icon::Music => {
            r#"<path d="M9 18V5l11-2v13M9 8l11-2"/><ellipse cx="6" cy="18" rx="3" ry="2.5"/><ellipse cx="17" cy="16" rx="3" ry="2.5"/>"#
        }
        Icon::Folder => r#"<path d="M3 6h7l2 3h9v11H3Z"/>"#,
        Icon::File => r#"<path d="M5 3h9l5 5v13H5Z"/><path d="M14 3v6h5M9 14h6M9 17h4"/>"#,
        Icon::Archive => r#"<path d="M4 7h16v14H4ZM3 3h18v4H3ZM10 11h4"/>"#,
        Icon::Crop => r#"<path d="M7 3v14h14M3 7h14v14"/>"#,
        Icon::Close => r#"<path d="m6 6 12 12M18 6 6 18"/>"#,
        Icon::Upload => r#"<path d="M12 16V3m-5 5 5-5 5 5M4 16v5h16v-5"/>"#,
        Icon::Check => r#"<path d="m5 12 4 4L19 6"/>"#,
        Icon::Edit => r#"<path d="m16 3 5 5-12 12-6 1 1-6Z"/><path d="m14 5 5 5"/>"#,
    };
    let color = format!(
        "rgb({},{},{})",
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8
    );
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="{color}" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">{shape}</svg>"#
    );
    svg(svg::Handle::from_memory(source.into_bytes()))
        .width(size)
        .height(size)
        .into()
}
