use cosmic_text::Buffer as CosmicBuffer;
use iced::advanced::clipboard::Clipboard;
use iced::event::{self, Event};
use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse;
use iced::advanced::renderer;
use iced::advanced::widget::{self, Widget};
use iced::advanced::Shell;
use iced::alignment;
use iced::widget::text_editor;
pub use iced::widget::text_editor::{Action, Content};
use iced::Color;
use iced::Element;
use iced::Length;
use iced::Padding;
use iced::Pixels;
use iced::Point;
use iced::Rectangle;
use iced::Renderer as IcedRenderer;
use iced::Size;
use iced::Theme as IcedTheme;
use iced::advanced::text::{LineHeight, Shaping, Text as PrimitiveText};
use iced::advanced::text::highlighter;
use iced::advanced::text::Renderer as TextRenderer;
use iced_graphics::text::Editor as GraphicsEditor;
use std::cell::{Ref, RefCell};

const DEFAULT_GUTTER_WIDTH: f32 = 64.0;
const DEFAULT_LINE_COLOR: Color = Color::from_rgba(0.7, 0.7, 0.7, 1.0);

pub struct TextEditor<'a, Message> {
    inner: text_editor::TextEditor<'a, highlighter::PlainText, Message>,
    content: &'a Content,
    base_padding: Padding,
    gutter_width: f32,
    line_color: Color,
}

impl<'a, Message> TextEditor<'a, Message> {
    pub fn new(content: &'a Content) -> Self {
        let base_padding = Padding::new(5.0);
        let gutter_width = DEFAULT_GUTTER_WIDTH;
        let mut inner = text_editor::TextEditor::new(content);
        let effective = add_gutter(base_padding, gutter_width);
        inner = inner.padding(effective);

        Self {
            inner,
            content,
            base_padding,
            gutter_width,
            line_color: DEFAULT_LINE_COLOR,
        }
    }

    pub fn on_action(mut self, on_edit: impl Fn(Action) -> Message + 'a) -> Self {
        self.inner = self.inner.on_action(on_edit);
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.inner = self.inner.height(height);
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.base_padding = padding.into();
        let effective = add_gutter(self.base_padding, self.gutter_width);
        self.inner = self.inner.padding(effective);
        self
    }

    pub fn line_number_color(mut self, color: Color) -> Self {
        self.line_color = color;
        self
    }
}

impl<'a, Message> Widget<Message, IcedTheme, IcedRenderer> for TextEditor<'a, Message>
where
    Message: 'a,
{
    fn tag(&self) -> widget::tree::Tag {
        self.inner.tag()
    }

    fn state(&self) -> widget::tree::State {
        self.inner.state()
    }

    fn size(&self) -> Size<Length> {
        self.inner.size()
    }

    fn layout(
        &self,
        tree: &mut widget::Tree,
        renderer: &IcedRenderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.inner.layout(tree, renderer, limits)
    }

    fn on_event(
        &mut self,
        tree: &mut widget::Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &IcedRenderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> event::Status {
        self.inner.on_event(
            tree,
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        )
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut IcedRenderer,
        theme: &IcedTheme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.inner
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
        draw_line_numbers(
            renderer,
            layout.bounds(),
            viewport,
            &self.base_padding,
            self.gutter_width,
            self.line_color,
            self.content,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &IcedRenderer,
    ) -> mouse::Interaction {
        self.inner
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }
}

impl<'a, Message> From<TextEditor<'a, Message>> for Element<'a, Message, IcedTheme, IcedRenderer>
where
    Message: 'a,
{
    fn from(editor: TextEditor<'a, Message>) -> Self {
        Element::new(editor)
    }
}

fn add_gutter(mut padding: Padding, gutter: f32) -> Padding {
    padding.left += gutter;
    padding
}

fn draw_line_numbers(
    renderer: &mut IcedRenderer,
    bounds: Rectangle,
    viewport: &Rectangle,
    base_padding: &Padding,
    gutter_width: f32,
    color: Color,
    content: &Content,
) {
    let _editor_ref = borrow_editor(content);
    let buffer = _editor_ref.buffer();
    let font_size = buffer.metrics().font_size;
    let line_height = buffer.metrics().line_height.max(1.0);
    let visible_lines = buffer.visible_lines().max(0) as usize;
    let scroll = buffer.scroll().max(0) as usize;
    let numbers = collect_visible_line_numbers(buffer, scroll, visible_lines);

    let start_x = bounds.x + base_padding.left;
    let start_y = bounds.y + base_padding.top;
    let text_size = Pixels(font_size);
    let font = renderer.default_font();

    for (index, line_number) in numbers.iter().enumerate() {
        let y = start_y + index as f32 * line_height;
        let label = line_number.to_string();
        let text = PrimitiveText {
            content: &label,
            bounds: Size::new(gutter_width, line_height),
            size: text_size,
            line_height: LineHeight::Absolute(Pixels(line_height)),
            font,
            horizontal_alignment: alignment::Horizontal::Right,
            vertical_alignment: alignment::Vertical::Top,
            shaping: Shaping::Basic,
        };

        renderer.fill_text(text, Point::new(start_x, y), color, *viewport);
    }
}

fn collect_visible_line_numbers(
    buffer: &CosmicBuffer,
    scroll: usize,
    visible_lines: usize,
) -> Vec<usize> {
    if buffer.lines.is_empty() {
        return vec![1];
    }

    let mut numbers = Vec::with_capacity(visible_lines.saturating_add(1));
    let mut remaining = scroll;
    let mut current_line = 0usize;
    let mut wrap_offset = 0usize;

    for (index, line) in buffer.lines.iter().enumerate() {
        let wraps = line
            .layout_opt()
            .as_ref()
            .map(|layout| layout.len())
            .unwrap_or(1);

        if remaining < wraps {
            current_line = index;
            wrap_offset = remaining;
            break;
        }

        remaining = remaining.saturating_sub(wraps);
        current_line = (index + 1).min(buffer.lines.len().saturating_sub(1));
        wrap_offset = 0;
    }

    let mut display_index = 0usize;
    let mut line_index = current_line;
    let mut local_offset = wrap_offset;
    let max_entries = visible_lines.saturating_add(1);

    while display_index < max_entries && line_index < buffer.lines.len() {
        let line = &buffer.lines[line_index];
        let wraps = line
            .layout_opt()
            .as_ref()
            .map(|layout| layout.len())
            .unwrap_or(1);

        let start = local_offset.min(wraps);
        local_offset = 0;

        for _ in start..wraps {
            numbers.push(line_index + 1);
            display_index += 1;

            if display_index >= max_entries {
                break;
            }
        }

        line_index += 1;
    }

    if numbers.is_empty() {
        numbers.push(current_line + 1);
    }

    numbers
}

#[repr(transparent)]
struct ContentRepr(RefCell<InternalRepr>);

#[repr(C)]
struct InternalRepr {
    editor: GraphicsEditor,
    is_dirty: bool,
}

fn borrow_editor(content: &Content) -> Ref<'_, GraphicsEditor> {
    unsafe {
        let repr = &*(content as *const Content as *const ContentRepr);
        Ref::map(repr.0.borrow(), |internal| &internal.editor)
    }
}
