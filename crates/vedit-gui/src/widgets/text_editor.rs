use cosmic_text::Buffer as CosmicBuffer;
use iced::advanced::clipboard::Clipboard;
use iced::event::{self, Event};
use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse;
use iced::advanced::renderer;
use iced::advanced::widget::{self, Widget, tree};
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
use iced::advanced::text::Highlighter as IcedHighlighter;
use iced::advanced::text::Renderer as TextRenderer;
use iced::advanced::Renderer as _;
use iced_graphics::text::Editor as GraphicsEditor;
use std::cell::{Cell, Ref, RefCell};
use std::rc::Rc;

const DEFAULT_GUTTER_WIDTH: f32 = 42.0;
const DEFAULT_LINE_COLOR: Color = Color::from_rgba(0.7, 0.7, 0.7, 1.0);
const GUTTER_TEXT_PADDING: f32 = 6.0;

pub struct TextEditor<'a, Message, H = highlighter::PlainText>
where
    H: IcedHighlighter,
{
    inner: text_editor::TextEditor<'a, H, Message>,
    content: &'a Content,
    base_padding: Padding,
    gutter_width: f32,
    line_color: Color,
    pointer_correction: Rc<Cell<f32>>,
    current_line_highlight: Option<Color>,
    indent_guides: Option<Color>,
    gutter_background: Option<Color>,
    show_minimap: bool,
    font_size: Option<Pixels>,
}

impl<'a, Message> TextEditor<'a, Message, highlighter::PlainText> {
    pub fn new(content: &'a Content) -> Self {
        let base_padding = Padding::new(5.0);
        let gutter_width = DEFAULT_GUTTER_WIDTH;
        let mut inner = text_editor::TextEditor::new(content);
        let effective = add_gutter(base_padding, gutter_width);
        inner = inner.padding(effective);
        let pointer_correction = Rc::new(Cell::new(pointer_correction_value(
            base_padding,
            gutter_width,
        )));

        Self {
            inner,
            content,
            base_padding,
            gutter_width,
            line_color: DEFAULT_LINE_COLOR,
            pointer_correction,
            current_line_highlight: None,
            indent_guides: None,
            gutter_background: None,
            show_minimap: false,
            font_size: None,
        }
    }

    pub fn highlight<NH>(
        self,
        settings: NH::Settings,
        to_format: fn(
            &NH::Highlight,
            &IcedTheme,
        ) -> highlighter::Format<<IcedRenderer as TextRenderer>::Font>,
    ) -> TextEditor<'a, Message, NH>
    where
        NH: IcedHighlighter,
    {
        TextEditor {
            inner: self.inner.highlight(settings, to_format),
            content: self.content,
            base_padding: self.base_padding,
            gutter_width: self.gutter_width,
            line_color: self.line_color,
            pointer_correction: Rc::clone(&self.pointer_correction),
            current_line_highlight: self.current_line_highlight,
            indent_guides: self.indent_guides,
            gutter_background: self.gutter_background,
            show_minimap: self.show_minimap,
            font_size: self.font_size,
        }
    }
}

impl<'a, Message, H> TextEditor<'a, Message, H>
where
    H: IcedHighlighter,
{
    pub fn on_action(mut self, on_edit: impl Fn(Action) -> Message + 'a) -> Self {
        let correction = Rc::clone(&self.pointer_correction);
        self.inner = self.inner.on_action(move |action| {
            let adjusted = adjust_action(action, correction.get());
            on_edit(adjusted)
        });
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
        self.pointer_correction
            .set(pointer_correction_value(self.base_padding, self.gutter_width));
        self
    }

    pub fn font<F>(mut self, font: F) -> Self
    where
        F: Into<<IcedRenderer as TextRenderer>::Font>,
    {
        self.inner = self.inner.font(font);
        self
    }

    pub fn line_number_color(mut self, color: Color) -> Self {
        self.line_color = color;
        self
    }

    pub fn current_line_highlight(mut self, color: Color) -> Self {
        self.current_line_highlight = Some(color);
        self
    }

    pub fn indent_guides(mut self, color: Color) -> Self {
        self.indent_guides = Some(color);
        self
    }

    pub fn gutter_background(mut self, color: Color) -> Self {
        self.gutter_background = Some(color);
        self
    }

    pub fn show_minimap(mut self, show: bool) -> Self {
        self.show_minimap = show;
        self
    }

    pub fn font_size(mut self, size: impl Into<Pixels>) -> Self {
        self.font_size = Some(size.into());
        self
    }
}

impl<'a, Message, H> Widget<Message, IcedTheme, IcedRenderer> for TextEditor<'a, Message, H>
where
    Message: 'a,
    H: IcedHighlighter,
{
    fn tag(&self) -> tree::Tag {
        self.inner.tag()
    }

    fn state(&self) -> tree::State {
        self.inner.state()
    }

    fn size(&self) -> Size<Length> {
        self.inner.size()
    }

    fn layout(
        &self,
        tree: &mut tree::Tree,
        renderer: &IcedRenderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.inner.layout(tree, renderer, limits)
    }

    fn on_event(
        &mut self,
        tree: &mut tree::Tree,
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
        tree: &tree::Tree,
        renderer: &mut IcedRenderer,
        theme: &IcedTheme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        self.inner
            .draw(tree, renderer, theme, style, layout, cursor, viewport);

        draw_line_numbers(
            renderer,
            bounds,
            viewport,
            &self.base_padding,
            self.gutter_width,
            self.line_color,
            self.content,
            self.font_size.map(|p| p.0),
        );

        // Minimap would be more complex, perhaps draw a small version on the right
        // For now, skip or add a placeholder
    }

    fn mouse_interaction(
        &self,
        tree: &tree::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &IcedRenderer,
    ) -> mouse::Interaction {
        self.inner
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }
}

impl<'a, Message, H> From<TextEditor<'a, Message, H>> for Element<'a, Message, IcedTheme, IcedRenderer>
where
    Message: 'a,
    H: IcedHighlighter,
{
    fn from(editor: TextEditor<'a, Message, H>) -> Self {
        Element::new(editor)
    }
}

fn add_gutter(mut padding: Padding, gutter: f32) -> Padding {
    padding.left += gutter;
    padding
}

fn pointer_correction_value(base_padding: Padding, gutter_width: f32) -> f32 {
    (base_padding.left + gutter_width) - base_padding.top
}

fn adjust_action(action: Action, pointer_correction: f32) -> Action {
    if pointer_correction.abs() <= f32::EPSILON {
        return action;
    }

    match action {
        Action::Click(position) => Action::Click(adjust_point(position, pointer_correction)),
        Action::Drag(position) => Action::Drag(adjust_point(position, pointer_correction)),
        other => other,
    }
}

fn adjust_point(position: Point, pointer_correction: f32) -> Point {
    Point::new(
        position.x - pointer_correction,
        position.y + pointer_correction,
    )
}

#[derive(Debug, Clone, Copy)]
pub struct ScrollMetrics {
    pub scroll: usize,
    pub visible_lines: usize,
    pub total_visual_lines: usize,
}

impl ScrollMetrics {
    pub fn max_scroll(&self) -> usize {
        self.total_visual_lines
            .saturating_sub(self.visible_lines)
    }
}

pub fn buffer_scroll_metrics(content: &Content) -> ScrollMetrics {
    let editor = borrow_editor(content);
    let buffer = editor.buffer();

    let visible_lines = buffer.visible_lines().max(0) as usize;
    let scroll = buffer.scroll().max(0) as usize;
    let total_visual_lines = count_visual_lines(buffer);

    ScrollMetrics {
        scroll,
        visible_lines,
        total_visual_lines,
    }
}

pub fn scroll_to(content: &mut Content, target: usize) {
    let metrics = buffer_scroll_metrics(content);
    let max_scroll = metrics.max_scroll();
    let clamped = target.min(max_scroll);
    let current = metrics.scroll;

    if clamped == current {
        return;
    }

    let delta = clamped as isize - current as isize;
    let delta = delta
        .clamp(i32::MIN as isize, i32::MAX as isize) as i32;

    if delta != 0 {
        content.perform(Action::Scroll { lines: delta });
    }
}

fn draw_line_numbers(
    renderer: &mut IcedRenderer,
    bounds: Rectangle,
    viewport: &Rectangle,
    base_padding: &Padding,
    gutter_width: f32,
    color: Color,
    content: &Content,
    font_size_override: Option<f32>,
) {
    let _editor_ref = borrow_editor(content);
    let buffer = _editor_ref.buffer();
    let font_size = font_size_override.unwrap_or(buffer.metrics().font_size);
    let line_height = buffer.metrics().line_height.max(1.0);
    let visible_lines = buffer.visible_lines().max(0) as usize;
    let scroll = buffer.scroll().max(0) as usize;
    let numbers = collect_visible_line_numbers(buffer, scroll, visible_lines);

    let gutter_right = bounds.x + base_padding.left + gutter_width;
    let start_y = bounds.y + base_padding.top;
    let text_size = Pixels(font_size);
    let font = renderer.default_font();
    let text_width = (gutter_width - GUTTER_TEXT_PADDING * 2.0).max(0.0);
    let start_x = (gutter_right - text_width - GUTTER_TEXT_PADDING).max(bounds.x);

    for (index, line_number) in numbers.iter().enumerate() {
        let y = start_y + index as f32 * line_height;
        let label = line_number.to_string();
        let text = PrimitiveText {
            content: &label,
            bounds: Size::new(text_width, line_height),
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

fn count_visual_lines(buffer: &CosmicBuffer) -> usize {
    buffer
        .lines
        .iter()
        .map(|line| {
            line
                .layout_opt()
                .as_ref()
                .map(|layout| layout.len())
                .unwrap_or(1)
        })
        .sum()
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
