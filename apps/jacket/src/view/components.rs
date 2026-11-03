use crate::app::{App, Message};
use crate::icons::Icon;
use crate::{artwork, icons};
use audio_engine::ArtworkMode;
use iced::widget::{container, image};
use iced::{Border, Element, Length};

#[derive(Clone, Copy)]
pub enum CoverContext {
    Source,
    Export,
}

pub fn cover(app: &App, size: f32, context: CoverContext) -> Element<'_, Message> {
    let p = app.presentation();
    let plan = app.preview_plan();
    let show = matches!(context, CoverContext::Source)
        || plan.is_some_and(|plan| !matches!(plan.artwork_mode, ArtworkMode::None));
    let imported = if matches!(context, CoverContext::Source) {
        app.cover.as_ref()
    } else {
        plan.filter(|plan| matches!(plan.artwork_mode, ArtworkMode::Shared))
            .and_then(|plan| {
                app.cover.as_ref().filter(|(asset, _)| {
                    plan.shared_artwork_asset.as_deref() == Some(asset.id.as_str())
                })
            })
    };
    let content: Element<'_, Message> = if !show {
        container(icons::view(Icon::Music, p.colors.text_muted, 24.0))
            .center(Length::Fill)
            .into()
    } else if let Some((_, path)) = imported {
        image(path)
            .width(size)
            .height(size)
            .content_fit(iced::ContentFit::Cover)
            .into()
    } else if app.example {
        image(artwork::example_cover())
            .width(size)
            .height(size)
            .content_fit(iced::ContentFit::Cover)
            .into()
    } else {
        let source = app.review.as_ref().and_then(artwork::source_url);
        let handle = match context {
            CoverContext::Source => app.source_cover.as_ref(),
            CoverContext::Export => app.preview_cover.as_ref().or_else(|| {
                if plan
                    .and_then(artwork::preview_url)
                    .is_some_and(|url| Some(url) == source)
                {
                    app.source_cover.as_ref()
                } else {
                    None
                }
            }),
        };
        if let Some(handle) = handle {
            image(handle.clone())
                .width(size)
                .height(size)
                .content_fit(iced::ContentFit::Cover)
                .into()
        } else {
            container(icons::view(Icon::Music, p.colors.text_muted, 24.0))
                .center(Length::Fill)
                .into()
        }
    };
    container(content)
        .width(size)
        .height(size)
        .clip(true)
        .style(move |_| container::Style {
            border: Border {
                color: p.colors.border,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..p.surface(p.colors.raised)
        })
        .into()
}

pub fn duration(seconds: f64) -> String {
    let seconds = if seconds.is_finite() && seconds >= 0.0 {
        seconds as u64
    } else {
        0
    };
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}
