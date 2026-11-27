use crate::app::Message;
use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{layout, mouse, overlay, renderer, Clipboard, Layout, Shell, Widget};
use iced::widget::{grid, Grid};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector};

const CARD_SPACING: f32 = 10.0;
const MIN_CARD_WIDTH: f32 = 180.0;

#[derive(Clone, Copy)]
pub(super) struct AlbumGridSize {
    pub columns: usize,
    pub card_width: f32,
}

impl AlbumGridSize {
    pub(super) fn for_width(width: f32) -> Self {
        let columns = ((width + CARD_SPACING) / (MIN_CARD_WIDTH + CARD_SPACING)) as usize;
        let columns = columns.max(1);
        let card_width = (width - CARD_SPACING * (columns - 1) as f32) / columns as f32;
        Self {
            columns,
            card_width,
        }
    }
}

pub(super) fn albums<'a>(
    cards: impl Iterator<Item = Element<'a, Message>>,
    columns: usize,
) -> Element<'a, Message> {
    Element::new(AlbumGrid {
        grid: grid(cards)
            .columns(columns)
            .height(Length::Shrink)
            .spacing(CARD_SPACING),
        columns,
    })
}

struct AlbumGrid<'a> {
    grid: Grid<'a, Message>,
    columns: usize,
}

impl Widget<Message, Theme, Renderer> for AlbumGrid<'_> {
    fn children(&self) -> Vec<Tree> {
        self.grid.children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.grid.diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.grid.size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let measured = self.grid.layout(tree, renderer, limits);
        let cards = measured.children();
        let card_height = cards
            .iter()
            .map(|card| card.size().height)
            .fold(0.0, f32::max);
        let row_count = cards.len().div_ceil(self.columns);
        let height =
            row_count as f32 * card_height + row_count.saturating_sub(1) as f32 * CARD_SPACING;

        let nodes = cards
            .iter()
            .enumerate()
            .map(|(index, card)| {
                let y = (index / self.columns) as f32 * (card_height + CARD_SPACING);
                stretch_card(card, card_height).move_to((card.bounds().x, y))
            })
            .collect();
        layout::Node::with_children(Size::new(measured.size().width, height), nodes)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.grid.operate(tree, layout, renderer, operation);
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
        self.grid.update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
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
        self.grid
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.grid
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.grid
            .overlay(tree, layout, renderer, viewport, translation)
    }
}

fn stretch_card(card: &layout::Node, height: f32) -> layout::Node {
    let [content] = card.children() else {
        unreachable!("an album button has one content column");
    };
    let [body, footer] = content.children() else {
        unreachable!("an album card contains a body and a release footer");
    };
    let extra_height = height - card.size().height;
    let content_bounds = content.bounds();
    let content = layout::Node::with_children(
        Size::new(content_bounds.width, content_bounds.height + extra_height),
        vec![
            body.clone(),
            footer.clone().translate(Vector::new(0.0, extra_height)),
        ],
    )
    .move_to((content_bounds.x, content_bounds.y));
    layout::Node::with_children(Size::new(card.size().width, height), vec![content])
}

#[cfg(test)]
mod tests {
    use super::{stretch_card, AlbumGridSize, MIN_CARD_WIDTH};
    use iced::advanced::layout::Node;
    use iced::Size;

    #[test]
    fn adds_columns_when_another_card_and_gap_fit() {
        for (width, expected_columns) in
            [(564.0, 3), (749.0, 3), (750.0, 4), (1116.0, 5), (1216.0, 6)]
        {
            let grid = AlbumGridSize::for_width(width);
            assert_eq!(grid.columns, expected_columns, "for {width} pixels");
            assert!(grid.card_width >= MIN_CARD_WIDTH);
        }
    }

    #[test]
    fn release_footers_align_when_the_card_bodies_have_different_heights() {
        let padding = 9.0;
        let gap = 4.0;
        let footer_height = 16.0;
        let card = |body_height| {
            let content = Node::with_children(
                Size::new(180.0, body_height + gap + footer_height),
                vec![
                    Node::new(Size::new(180.0, body_height)),
                    Node::new(Size::new(80.0, footer_height)).move_to((0.0, body_height + gap)),
                ],
            );
            Node::container(content, padding.into())
        };
        let wrapped = card(200.0);
        let height = wrapped.size().height;

        for original in [card(182.0), wrapped] {
            let stretched = stretch_card(&original, height);
            let content = &stretched.children()[0];
            let body = &content.children()[0];
            let footer = content.children()[1].bounds();

            assert_eq!(stretched.size().height, height);
            assert_eq!(body.bounds(), original.children()[0].children()[0].bounds());
            assert_eq!(
                content.bounds().y + footer.y + footer.height,
                height - padding
            );
        }
    }
}
