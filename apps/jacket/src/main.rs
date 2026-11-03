//! Native entry point. Business logic and desktop services live in independent crates.
mod app;
mod artwork;
mod crop_view;
mod editor;
mod example;
mod platform;
mod settings;
mod style;
mod track_list;
mod view;

use app::App;
use iced::window;

fn main() -> iced::Result {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
    let icon = image::load_from_memory(include_bytes!(
        "../../../branding/jacket/png/jacket-icon-256.png"
    ))
    .ok()
    .and_then(|image| {
        let image = image.into_rgba8();
        window::icon::from_rgba(image.to_vec(), image.width(), image.height()).ok()
    });
    iced::application(App::new, App::update, App::view)
        .settings(iced::Settings {
            id: Some("com.moonlark.audio-dl".into()),
            ..iced::Settings::default()
        })
        .title("Jacket — From link to neatly packed")
        .theme(App::theme)
        .font(include_bytes!("../../../assets/fonts/Geist.ttf").as_slice())
        .default_font(iced::Font::with_name("Geist"))
        .window(window::Settings {
            icon,
            size: if std::env::args().any(|arg| arg == "--compact") {
                iced::Size::new(600.0, 600.0)
            } else {
                iced::Size::new(1120.0, 820.0)
            },
            min_size: Some(iced::Size::new(600.0, 600.0)),
            position: window::Position::Centered,
            ..window::Settings::default()
        })
        .exit_on_close_request(false)
        .subscription(App::subscription)
        .antialiasing(true)
        .run()
}
