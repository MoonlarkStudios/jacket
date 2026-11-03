use crate::app::{App, Message};
use iced::widget::{container, image, mouse_area, stack, svg};
use iced::{Element, Point};

pub const WIDTH: f32 = 360.0;
pub const HEIGHT: f32 = 260.0;

pub fn view(app: &App) -> Element<'_, Message> {
    let Some((asset, path)) = &app.cover else {
        return container("").into();
    };
    let size = app
        .crop_size
        .parse::<u32>()
        .unwrap_or(asset.width.min(asset.height))
        .min(asset.width.min(asset.height));
    let x = app
        .crop_x
        .parse::<u32>()
        .unwrap_or(0)
        .min(asset.width - size);
    let y = app
        .crop_y
        .parse::<u32>()
        .unwrap_or(0)
        .min(asset.height - size);
    let stroke = asset.width as f32 / WIDTH * 2.0;
    let overlay = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}"><path d="M0 0H{width}V{height}H0Z M{x} {y}h{size}v{size}h-{size}Z" fill="#000" fill-opacity=".48" fill-rule="evenodd"/><rect x="{x}" y="{y}" width="{size}" height="{size}" fill="none" stroke="#F4EFE5" stroke-width="{stroke}"/></svg>"##,
        width = asset.width,
        height = asset.height
    );
    mouse_area(stack![
        image(path)
            .width(WIDTH)
            .height(HEIGHT)
            .content_fit(iced::ContentFit::Contain),
        svg(svg::Handle::from_memory(overlay.into_bytes()))
            .width(WIDTH)
            .height(HEIGHT)
            .content_fit(iced::ContentFit::Contain),
    ])
    .on_move(Message::CropPointer)
    .on_press(Message::CropDown)
    .on_release(Message::CropUp)
    .on_exit(Message::CropUp)
    .into()
}

/// Convert the letterboxed preview pointer into a square contained in the source image.
pub fn origin(point: Point, width: u32, height: u32, size: u32) -> (u32, u32) {
    let size = size.min(width.min(height));
    if width == 0 || height == 0 {
        return (0, 0);
    }
    let scale = (WIDTH / width as f32).min(HEIGHT / height as f32);
    let left = (WIDTH - width as f32 * scale) / 2.0;
    let top = (HEIGHT - height as f32 * scale) / 2.0;
    let x = ((point.x - left) / scale - size as f32 / 2.0)
        .round()
        .clamp(0.0, (width - size) as f32);
    let y = ((point.y - top) / scale - size as f32 / 2.0)
        .round()
        .clamp(0.0, (height - size) as f32);
    (x as u32, y as u32)
}

#[cfg(test)]
mod tests {
    use super::origin;
    use iced::Point;

    #[test]
    fn letterboxing_and_dragging_beyond_edges_keep_the_crop_inside_the_image() {
        assert_eq!(origin(Point::new(180.0, 130.0), 1200, 600, 300), (450, 150));
        assert_eq!(origin(Point::new(-20.0, -30.0), 1200, 600, 300), (0, 0));
        assert_eq!(origin(Point::new(800.0, 800.0), 1200, 600, 300), (900, 300));
        assert_eq!(origin(Point::new(180.0, 130.0), 600, 1200, 600), (0, 300));
    }
}
