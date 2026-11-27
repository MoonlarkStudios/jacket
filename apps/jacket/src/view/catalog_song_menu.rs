use super::focus_button;
use crate::app::{song_album_id, App, CatalogAction, Message};
use crate::icons::{self, Icon};
use crate::style::CatalogSurface;
use iced::advanced::widget::{tree, Operation, Tree};
use iced::advanced::{layout, mouse, overlay, renderer, Clipboard, Layout, Shell, Widget};
use iced::keyboard::{self, key::Named, Key};
use iced::widget::overlay::menu;
use iced::widget::{button, text, tooltip};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector};
use std::time::Duration;

const MENU_WIDTH: f32 = 136.0;

pub(super) fn view(app: &App, id: &str) -> Element<'static, Message> {
    let presentation = app.presentation();
    let open = app.catalog.song_menu_open(id) && !app.busy() && !app.catalog.confirm_replace;
    let trigger = button(icons::view(
        Icon::MoreVertical,
        presentation.colors.text,
        18.0,
    ))
    .padding(7);
    let trigger = if open {
        trigger.style(presentation.choice(true))
    } else {
        trigger.style(presentation.catalog_result(CatalogSurface::Flat))
    };
    let trigger = focus_button::wrap_inset(
        song_album_id(id),
        trigger,
        (!app.busy() && !app.catalog.confirm_replace)
            .then(|| Message::Catalog(CatalogAction::ToggleSongMenu(id.into()))),
        presentation.colors.accent,
        presentation.colors.background,
    );
    Element::new(SongMenu {
        trigger: tooltip(
            trigger,
            text("More song actions").size(12),
            tooltip::Position::Left,
        )
        .delay(Duration::from_millis(500))
        .into(),
        open,
        select: Message::Catalog(CatalogAction::OpenSongAlbum(id.into())),
        style: Box::new(move |_| menu::Style {
            background: presentation.colors.raised.into(),
            border: presentation.panel().border,
            text_color: presentation.colors.text,
            selected_text_color: presentation.colors.text,
            selected_background: presentation.derived.selection_fill.into(),
            shadow: iced::Shadow {
                color: iced::Color::BLACK.scale_alpha(0.25),
                offset: Vector::new(0.0, 3.0),
                blur_radius: 9.0,
            },
        }),
    })
}

struct SongMenu {
    trigger: Element<'static, Message>,
    open: bool,
    select: Message,
    style: menu::StyleFn<'static, Theme>,
}

struct State {
    menu: menu::State,
    selection: Option<usize>,
}

impl Widget<Message, Theme, Renderer> for SongMenu {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            menu: menu::State::new(),
            selection: Some(0),
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.trigger)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.trigger));
    }

    fn size(&self) -> Size<Length> {
        self.trigger.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.trigger
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.trigger
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
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
        self.trigger.as_widget_mut().update(
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
        self.trigger.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.trigger.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        if !self.open {
            return self.trigger.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                translation,
            );
        }
        let bounds = layout.bounds() + translation;
        if !viewport.intersects(&bounds) {
            return None;
        }
        let state = tree.state.downcast_mut::<State>();
        let select = &self.select;
        let menu = menu::Menu::new(
            &mut state.menu,
            &["View album"],
            &mut state.selection,
            |_| select.clone(),
            None,
            &self.style,
        )
        .width(MENU_WIDTH)
        .padding([9, 12])
        .text_size(13)
        .overlay(
            layout.position() + translation + Vector::new(bounds.width - MENU_WIDTH, 0.0),
            *viewport,
            bounds.height,
            Length::Shrink,
        );
        Some(overlay::Element::new(Box::new(MenuOverlay {
            menu,
            select,
        })))
    }
}

struct MenuOverlay<'a> {
    menu: overlay::Element<'a, Message, Theme, Renderer>,
    select: &'a Message,
}

impl overlay::Overlay<Message, Theme, Renderer> for MenuOverlay<'_> {
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        self.menu.as_overlay_mut().layout(renderer, bounds)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        self.menu
            .as_overlay()
            .draw(renderer, theme, style, layout, cursor);
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.menu
            .as_overlay()
            .mouse_interaction(layout, cursor, renderer)
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        match event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(Named::Enter | Named::Space),
                modifiers,
                repeat,
                ..
            }) if modifiers.is_empty() => {
                if !repeat {
                    shell.publish(self.select.clone());
                }
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonPressed(_))
            | Event::Touch(iced::touch::Event::FingerPressed { .. })
                if !cursor.is_over(layout.bounds()) =>
            {
                shell.publish(Message::Catalog(CatalogAction::DismissSongMenu));
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::WheelScrolled { .. })
            | Event::Window(iced::window::Event::Unfocused) => {
                shell.publish(Message::Catalog(CatalogAction::DismissSongMenu));
            }
            _ => self
                .menu
                .as_overlay_mut()
                .update(event, layout, cursor, renderer, clipboard, shell),
        }
    }
}
