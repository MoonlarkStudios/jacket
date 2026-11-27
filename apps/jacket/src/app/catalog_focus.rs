use super::Message;
use iced::advanced::widget::{operate, operation, Id, Operation};
use iced::{Rectangle, Task, Vector};

pub(crate) const CATALOG_SCROLL_ID: &str = "album-browser";
const FOCUS_MARGIN: f32 = 4.0;

pub(super) fn reveal() -> Task<Message> {
    reveal_in(CATALOG_SCROLL_ID)
}

pub(super) fn reveal_in(id: &'static str) -> Task<Message> {
    operate(RevealFocused {
        id,
        viewport: None,
        next_viewport: None,
        scroll_by: 0.0,
    })
    .discard()
}

#[derive(Clone, Copy)]
struct Viewport {
    bounds: Rectangle,
    scroll_y: f32,
}

impl Viewport {
    fn scroll_to_reveal(self, focused: Rectangle) -> f32 {
        let top = self.bounds.y + FOCUS_MARGIN;
        let bottom = self.bounds.y + self.bounds.height - FOCUS_MARGIN;
        let y = focused.y - self.scroll_y;
        let height = focused.height.min((bottom - top).max(0.0));
        if y < top {
            y - top
        } else {
            (y + height - bottom).max(0.0)
        }
    }
}

struct RevealFocused {
    id: &'static str,
    viewport: Option<Viewport>,
    next_viewport: Option<Viewport>,
    scroll_by: f32,
}

impl Operation for RevealFocused {
    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        _content_bounds: Rectangle,
        translation: Vector,
        _state: &mut dyn operation::Scrollable,
    ) {
        if id == Some(&Id::new(self.id)) {
            self.next_viewport = Some(Viewport {
                bounds,
                scroll_y: translation.y,
            });
        }
    }

    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation)) {
        let parent = self.viewport;
        self.viewport = self.next_viewport.take().or(parent);
        visit(self);
        self.viewport = parent;
    }

    fn focusable(
        &mut self,
        _id: Option<&Id>,
        bounds: Rectangle,
        state: &mut dyn operation::Focusable,
    ) {
        if let Some(viewport) = self.viewport.filter(|_| state.is_focused()) {
            self.scroll_by = viewport.scroll_to_reveal(bounds);
        }
    }

    fn finish(&self) -> operation::Outcome<()> {
        if self.scroll_by == 0.0 {
            return operation::Outcome::None;
        }
        operation::Outcome::Chain(Box::new(operation::scrollable::scroll_by(
            Id::new(self.id),
            operation::scrollable::AbsoluteOffset {
                x: 0.0,
                y: self.scroll_by,
            },
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::Viewport;
    use iced::Rectangle;

    #[test]
    fn focus_scrolls_only_as_far_as_needed_in_either_direction() {
        let viewport = Viewport {
            bounds: Rectangle {
                x: 0.0,
                y: 200.0,
                width: 600.0,
                height: 400.0,
            },
            scroll_y: 120.0,
        };
        let row = |y| Rectangle {
            x: 18.0,
            y,
            width: 564.0,
            height: 56.0,
        };
        assert_eq!(viewport.scroll_to_reveal(row(300.0)), -24.0);
        assert_eq!(viewport.scroll_to_reveal(row(400.0)), 0.0);
        assert_eq!(viewport.scroll_to_reveal(row(700.0)), 40.0);
    }
}
