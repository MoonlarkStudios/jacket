//! Add Tab and keyboard activation to a stock iced button.
use crate::app::Message;
use iced::advanced::widget::{operation, tree, Id, Operation, Tree};
use iced::advanced::{layout, mouse, renderer, Clipboard, Layout, Renderer as _, Shell, Widget};
use iced::keyboard::{self, key::Named, Key};
use iced::widget::Button;
use iced::{Border, Color, Element, Event, Length, Rectangle, Renderer, Size, Theme};

pub fn wrap<'a>(
    id: impl Into<Id>,
    button: Button<'a, Message>,
    action: Option<Message>,
    ring: Color,
) -> Element<'a, Message> {
    Element::new(FocusButton {
        id: id.into(),
        child: button.on_press_maybe(action.clone()).into(),
        action,
        ring,
    })
}

struct FocusButton<'a> {
    id: Id,
    child: Element<'a, Message>,
    action: Option<Message>,
    ring: Color,
}

#[derive(Default)]
struct State {
    focused: bool,
    space_pressed: bool,
}

impl operation::Focusable for State {
    fn is_focused(&self) -> bool {
        self.focused
    }

    fn focus(&mut self) {
        self.focused = true;
    }

    fn unfocus(&mut self) {
        *self = Self::default();
    }
}

impl Widget<Message, Theme, Renderer> for FocusButton<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.child)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.children[0].diff(&self.child);
        if self.action.is_none() {
            *tree.state.downcast_mut::<State>() = State::default();
        }
    }

    fn size(&self) -> Size<Length> {
        self.child.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.child
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if self.action.is_some() {
            operation.focusable(
                Some(&self.id),
                layout.bounds(),
                tree.state.downcast_mut::<State>(),
            );
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        if let Some(action) = &self.action {
            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    let focused = cursor.is_over(layout.bounds()) && cursor.is_over(*viewport);
                    if focused != state.focused {
                        *state = State {
                            focused,
                            space_pressed: false,
                        };
                        shell.request_redraw();
                    }
                }
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: Key::Named(key),
                    modifiers,
                    repeat,
                    ..
                }) if state.focused && !shell.is_event_captured() && modifiers.is_empty() => {
                    match key {
                        Named::Enter => {
                            if !repeat {
                                shell.publish(action.clone());
                            }
                            shell.capture_event();
                        }
                        Named::Space => {
                            state.space_pressed = true;
                            shell.capture_event();
                        }
                        _ => (),
                    }
                }
                Event::Keyboard(keyboard::Event::KeyReleased {
                    key: Key::Named(Named::Space),
                    ..
                }) if std::mem::take(&mut state.space_pressed) => {
                    if state.focused && !shell.is_event_captured() {
                        shell.publish(action.clone());
                        shell.capture_event();
                    }
                }
                Event::Window(iced::window::Event::Unfocused) => state.space_pressed = false,
                _ => (),
            }
        }
        self.child.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.child.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        if self.action.is_some() && tree.state.downcast_ref::<State>().focused {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds().expand(3),
                    border: Border {
                        color: self.ring,
                        width: 2.0,
                        radius: 7.0.into(),
                    },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.child.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }
}
