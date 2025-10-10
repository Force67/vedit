use cosmic_text::Buffer as CosmicBuffer;
use iced::advanced::clipboard::Clipboard;
use iced::event::{self, Event};
use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse;
use iced::advanced::renderer;
use iced::advanced::widget::{Widget, tree};
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
use crate::utils::pool::{get_pooled_usize_vec, get_pooled_string, return_usize_vec, return_string};

const DEFAULT_GUTTER_WIDTH: f32 = 60.0;
const DEFAULT_LINE_COLOR: Color = Color::from_rgba(0.7, 0.7, 0.7, 1.0);
const GUTTER_TEXT_PADDING: f32 = 12.0;
const GUTTER_BACKGROUND: Color = Color::from_rgba(0.05, 0.05, 0.05, 1.0);
const GUTTER_BORDER_COLOR: Color = Color::from_rgba(0.3, 0.3, 0.3, 1.0);
const GUTTER_BORDER_WIDTH: f32 = 1.0;

/// Cached line number data to avoid recomputation
#[derive(Debug, Clone)]
struct CachedLineNumbers {
    numbers: Vec<usize>,
    scroll: usize,
    visible_lines: usize,
    total_lines: usize,
    font_size: f32,
    line_height: f32,
    cached_text_batches: Vec<(String, f32, f32)>, // (text, x, y) positions
    batch_valid: bool,
}

/// Incremental line number state for fast scrolling
#[derive(Debug, Clone)]
struct IncrementalLineState {
    // The buffer line number and wrap offset for the first visible line
    start_buffer_line: usize,
    start_wrap_offset: usize,
    // Cached layout results for buffer lines
    line_wraps: Vec<usize>, // wraps per buffer line
    buffer_version: u64,
    cached_scroll: usize, // Last processed scroll position
    last_update_time: std::time::Instant, // Throttle scroll processing
}

impl IncrementalLineState {
    fn new() -> Self {
        Self {
            start_buffer_line: 0,
            start_wrap_offset: 0,
            line_wraps: Vec::new(),
            buffer_version: 0,
            cached_scroll: 0,
            last_update_time: std::time::Instant::now(),
        }
    }

    fn is_valid(&self, buffer: &CosmicBuffer, current_scroll: usize) -> bool {
        let now = std::time::Instant::now();
        // Throttle scroll processing to 120Hz (8ms) for smoother scrolling
        let time_since_last = now.duration_since(self.last_update_time).as_millis() < 8;

        self.buffer_version == get_buffer_version(buffer)
            && (self.cached_scroll == current_scroll || time_since_last)
    }

    fn update(&mut self, buffer: &CosmicBuffer, scroll: usize) {
        // Skip update if scroll position hasn't changed significantly
        let scroll_delta = if self.cached_scroll > scroll {
            self.cached_scroll - scroll
        } else {
            scroll - self.cached_scroll
        };

        // Only update if scroll changed by more than 1 line or it's been long enough
        if scroll_delta > 1 || std::time::Instant::now().duration_since(self.last_update_time).as_millis() > 16 {
            self.cached_scroll = scroll;
            self.last_update_time = std::time::Instant::now();

            self.buffer_version = get_buffer_version(buffer);

            // Update line wraps cache with layout caching
            if self.line_wraps.len() != buffer.lines.len() {
                self.line_wraps.clear();
                self.line_wraps.reserve(buffer.lines.len());

                for line in &buffer.lines {
                    // Cache layout results to avoid repeated text shaping
                    let wraps = line
                        .layout_opt()
                        .as_ref()
                        .map(|layout| layout.len())
                        .unwrap_or(1);
                    self.line_wraps.push(wraps);
                }
            }

            // Find the starting buffer line and wrap offset for current scroll
            let mut remaining = scroll;
            for (line_index, &wraps) in self.line_wraps.iter().enumerate() {
                if remaining < wraps {
                    self.start_buffer_line = line_index;
                    self.start_wrap_offset = remaining;
                    break;
                }
                remaining = remaining.saturating_sub(wraps);
                self.start_buffer_line = (line_index + 1).min(buffer.lines.len().saturating_sub(1));
                self.start_wrap_offset = 0;
            }
        }
    }

    fn get_visible_lines(&self, start_scroll: usize, visible_lines: usize, total_lines: usize) -> Vec<usize> {
        let mut result = Vec::with_capacity(visible_lines.saturating_add(1));
        let mut current_buffer_line = self.start_buffer_line;
        let mut current_wrap_offset = self.start_wrap_offset;
        let mut display_index = 0;

        while display_index < visible_lines && current_buffer_line < self.line_wraps.len() {
            let wraps = self.line_wraps[current_buffer_line];

            for wrap_idx in current_wrap_offset..wraps {
                result.push(current_buffer_line + 1);
                display_index += 1;

                if display_index >= visible_lines {
                    break;
                }
            }

            current_buffer_line += 1;
            current_wrap_offset = 0;
        }

        if result.is_empty() {
            result.push(1);
        }

        result
    }
}

impl CachedLineNumbers {
    fn new() -> Self {
        Self {
            numbers: Vec::new(),
            scroll: 0,
            visible_lines: 0,
            total_lines: 0,
            font_size: 0.0,
            line_height: 0.0,
            cached_text_batches: Vec::new(),
            batch_valid: false,
        }
    }

    fn is_valid(&self, scroll: usize, visible_lines: usize, total_lines: usize, font_size: f32, line_height: f32) -> bool {
        self.scroll == scroll
            && self.visible_lines == visible_lines
            && self.total_lines == total_lines
            && (self.font_size - font_size).abs() < f32::EPSILON
            && (self.line_height - line_height).abs() < f32::EPSILON
    }

    fn update(&mut self, numbers: Vec<usize>, scroll: usize, visible_lines: usize, total_lines: usize, font_size: f32, line_height: f32) {
        self.numbers = numbers;
        self.scroll = scroll;
        self.visible_lines = visible_lines;
        self.total_lines = total_lines;
        self.font_size = font_size;
        self.line_height = line_height;
        self.batch_valid = false; // Invalidate cached batches
    }

    fn get_or_create_text_batches(&mut self, bounds: Rectangle, base_padding: &Padding, gutter_width: f32, line_height: f32) -> &[(String, f32, f32)] {
        if !self.batch_valid {
            self.cached_text_batches.clear();

            let gutter_right = bounds.x + base_padding.left + gutter_width;
            let start_y = bounds.y + base_padding.top;
            let text_width = (gutter_width - GUTTER_TEXT_PADDING * 2.0).max(0.0);
            let start_x = (gutter_right - text_width - GUTTER_TEXT_PADDING).max(bounds.x);

            self.cached_text_batches.reserve(self.numbers.len());

            for (index, line_number) in self.numbers.iter().enumerate() {
                let y = start_y + index as f32 * line_height;
                let text = line_number.to_string();
                self.cached_text_batches.push((text, start_x, y));
            }

            self.batch_valid = true;
        }

        &self.cached_text_batches
    }
}

/// Cached line metrics to avoid repeated cosmic-text queries
#[derive(Debug, Clone)]
struct CachedLineMetrics {
    line_height: f32,
    font_size: f32,
    visible_lines: usize,
    total_visual_lines: usize,
    buffer_version: u64, // Track buffer changes
    current_scroll: usize, // Cache current scroll position
    last_render_time: std::time::Instant, // Throttle updates
}

impl CachedLineMetrics {
    fn new() -> Self {
        Self {
            line_height: 0.0,
            font_size: 0.0,
            visible_lines: 0,
            total_visual_lines: 0,
            buffer_version: 0,
            current_scroll: 0,
            last_render_time: std::time::Instant::now(),
        }
    }

    fn needs_update(&self, buffer: &CosmicBuffer, font_size: f32, scroll: usize) -> bool {
        let now = std::time::Instant::now();
        // Throttle updates to 60Hz (16ms) to prevent excessive calculations
        let time_since_last = now.duration_since(self.last_render_time).as_millis() < 16;

        if time_since_last {
            return false; // Skip update if too recent
        }

        self.buffer_version != get_buffer_version(buffer)
            || (self.font_size - font_size).abs() > f32::EPSILON
            || self.current_scroll != scroll
    }

    fn is_valid(&self, buffer: &CosmicBuffer, font_size: f32) -> bool {
        (self.font_size - font_size).abs() < f32::EPSILON
            && self.buffer_version == get_buffer_version(buffer)
    }

    fn update(&mut self, buffer: &CosmicBuffer, font_size: f32, scroll: usize) {
        self.line_height = buffer.metrics().line_height.max(1.0);
        self.font_size = font_size;
        self.visible_lines = buffer.visible_lines().max(0) as usize;
        self.total_visual_lines = count_visual_lines(buffer);
        self.buffer_version = get_buffer_version(buffer);
        self.current_scroll = scroll;
        self.last_render_time = std::time::Instant::now();
    }
}

// Simple versioning for buffer changes (using address as heuristic)
fn get_buffer_version(_buffer: &CosmicBuffer) -> u64 {
    // In a real implementation, you'd want proper version tracking
    // For now, we use the buffer's memory address as a heuristic
    _buffer as *const _ as u64
}

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
    cached_line_numbers: Rc<RefCell<CachedLineNumbers>>,
    cached_line_metrics: Rc<RefCell<CachedLineMetrics>>,
    cached_scroll_metrics: Rc<RefCell<CachedScrollMetrics>>,
    incremental_line_state: Rc<RefCell<IncrementalLineState>>,
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
            gutter_background: Some(GUTTER_BACKGROUND),
            show_minimap: false,
            font_size: None,
            cached_line_numbers: Rc::new(RefCell::new(CachedLineNumbers::new())),
            cached_line_metrics: Rc::new(RefCell::new(CachedLineMetrics::new())),
            cached_scroll_metrics: Rc::new(RefCell::new(CachedScrollMetrics::new())),
            incremental_line_state: Rc::new(RefCell::new(IncrementalLineState::new())),
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
            cached_line_numbers: Rc::clone(&self.cached_line_numbers),
            cached_line_metrics: Rc::clone(&self.cached_line_metrics),
            cached_scroll_metrics: Rc::clone(&self.cached_scroll_metrics),
            incremental_line_state: Rc::clone(&self.incremental_line_state),
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

    /// Get cached scroll metrics, updating cache if needed
    pub fn cached_scroll_metrics(&self) -> ScrollMetrics {
        let editor_ref = borrow_editor(self.content);
        let buffer = editor_ref.buffer();
        let current_scroll = buffer.scroll().max(0) as usize;

        let mut cache = self.cached_scroll_metrics.borrow_mut();
        if !cache.is_valid(buffer, current_scroll) {
            cache.update(buffer);
        }
        cache.metrics
    }

    /// Invalidate caches when content changes
    pub fn invalidate_caches(&self) {
        // Force cache invalidation by updating version
        let editor_ref = borrow_editor(self.content);
        let buffer = editor_ref.buffer();
        let version = get_buffer_version(buffer);

        // Update line metrics cache with new version
        let mut metrics_cache = self.cached_line_metrics.borrow_mut();
        metrics_cache.buffer_version = version;

        // Update scroll metrics cache with new version
        let mut scroll_cache = self.cached_scroll_metrics.borrow_mut();
        scroll_cache.buffer_version = version;

        // Clear line numbers cache entirely as it's more sensitive to changes
        let mut line_numbers_cache = self.cached_line_numbers.borrow_mut();
        line_numbers_cache.numbers.clear();

        // Reset incremental state on content changes
        let mut incremental_state = self.incremental_line_state.borrow_mut();
        incremental_state.buffer_version = 0; // Force refresh
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

        draw_line_numbers_optimized_with_background(
            renderer,
            bounds,
            viewport,
            &self.base_padding,
            self.gutter_width,
            self.line_color,
            self.gutter_background.unwrap_or(GUTTER_BACKGROUND),
            self.content,
            self.font_size.map(|p| p.0),
            &self.cached_line_numbers,
            &self.cached_line_metrics,
            &self.incremental_line_state,
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
        if let Some(position) = cursor.position_over(layout.bounds()) {
            // Check if cursor is in gutter area
            let gutter_right = layout.bounds().x + self.base_padding.left + self.gutter_width;

            if position.x < gutter_right {
                // Cursor is in gutter area - VSCode typically shows pointer cursor here
                mouse::Interaction::Pointer
            } else {
                // Cursor is in content area - delegate to inner text editor
                self.inner
                    .mouse_interaction(tree, layout, cursor, viewport, renderer)
            }
        } else {
            // Cursor is not over widget
            mouse::Interaction::default()
        }
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

/// Cached scroll metrics to avoid repeated buffer queries
#[derive(Debug, Clone)]
struct CachedScrollMetrics {
    metrics: ScrollMetrics,
    buffer_version: u64,
    last_scroll: Option<usize>,
}

impl CachedScrollMetrics {
    fn new() -> Self {
        Self {
            metrics: ScrollMetrics {
                scroll: 0,
                visible_lines: 0,
                total_visual_lines: 0,
            },
            buffer_version: 0,
            last_scroll: None,
        }
    }

    fn is_valid(&self, buffer: &CosmicBuffer, current_scroll: usize) -> bool {
        self.buffer_version == get_buffer_version(buffer)
            && self.last_scroll.map_or(false, |last| last == current_scroll)
    }

    fn update(&mut self, buffer: &CosmicBuffer) {
        let visible_lines = buffer.visible_lines().max(0) as usize;
        let scroll = buffer.scroll().max(0) as usize;
        let total_visual_lines = count_visual_lines(buffer);

        self.metrics = ScrollMetrics {
            scroll,
            visible_lines,
            total_visual_lines,
        };
        self.buffer_version = get_buffer_version(buffer);
        self.last_scroll = Some(scroll);
    }
}

pub fn buffer_scroll_metrics(content: &Content) -> ScrollMetrics {
    buffer_scroll_metrics_optimized(content)
}

/// Optimized scroll metrics with caching for external use
pub fn buffer_scroll_metrics_optimized(content: &Content) -> ScrollMetrics {
    thread_local! {
        static SCROLL_CACHE: std::cell::RefCell<CachedScrollMetrics> = std::cell::RefCell::new(CachedScrollMetrics::new());
    }

    let editor = borrow_editor(content);
    let buffer = editor.buffer();
    let current_scroll = buffer.scroll().max(0) as usize;

    SCROLL_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if !cache.is_valid(buffer, current_scroll) {
            cache.update(buffer);
        }
        cache.metrics
    })
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

    // Draw gutter background (VSCode-style)
    let gutter_bounds = Rectangle {
        x: bounds.x,
        y: bounds.y,
        width: base_padding.left + gutter_width,
        height: bounds.height,
    };

    // Fill gutter background
    renderer.fill_quad(
        renderer::Quad {
            bounds: gutter_bounds,
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
            shadow: iced::Shadow::default(),
        },
        GUTTER_BACKGROUND,
    );

    // Draw gutter border (separator line)
    let border_bounds = Rectangle {
        x: gutter_bounds.x + gutter_bounds.width - GUTTER_BORDER_WIDTH,
        y: gutter_bounds.y,
        width: GUTTER_BORDER_WIDTH,
        height: gutter_bounds.height,
    };

    renderer.fill_quad(
        renderer::Quad {
            bounds: border_bounds,
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
            shadow: iced::Shadow::default(),
        },
        GUTTER_BORDER_COLOR,
    );

    // Calculate text positioning
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

fn draw_line_numbers_optimized_with_background(
    renderer: &mut IcedRenderer,
    bounds: Rectangle,
    viewport: &Rectangle,
    base_padding: &Padding,
    gutter_width: f32,
    color: Color,
    background_color: Color,
    content: &Content,
    font_size_override: Option<f32>,
    cached_line_numbers: &Rc<RefCell<CachedLineNumbers>>,
    cached_line_metrics: &Rc<RefCell<CachedLineMetrics>>,
    incremental_line_state: &Rc<RefCell<IncrementalLineState>>,
) {
    let _editor_ref = borrow_editor(content);
    let buffer = _editor_ref.buffer();
    let font_size = font_size_override.unwrap_or(buffer.metrics().font_size);
    let scroll = buffer.scroll().max(0) as usize;

    // Check if line metrics cache needs update with throttling
    let mut metrics_cache = cached_line_metrics.borrow_mut();
    if metrics_cache.needs_update(buffer, font_size, scroll) {
        metrics_cache.update(buffer, font_size, scroll);
    }
    let line_height = metrics_cache.line_height;
    let visible_lines = metrics_cache.visible_lines;
    let total_lines = metrics_cache.total_visual_lines;

    // Use incremental line state for fast scroll calculations
    let mut incremental_state = incremental_line_state.borrow_mut();
    if !incremental_state.is_valid(buffer, scroll) {
        incremental_state.update(buffer, scroll);
    }

    // Get line numbers using the incremental approach
    let numbers = incremental_state.get_visible_lines(scroll, visible_lines, total_lines);

    // Update the traditional cache as well for compatibility
    let mut line_numbers_cache = cached_line_numbers.borrow_mut();
    if !line_numbers_cache.is_valid(scroll, visible_lines, total_lines, font_size, line_height) {
        line_numbers_cache.update(numbers.clone(), scroll, visible_lines, total_lines, font_size, line_height);
    }

    // Draw gutter background (VSCode-style)
    let gutter_bounds = Rectangle {
        x: bounds.x,
        y: bounds.y,
        width: base_padding.left + gutter_width,
        height: bounds.height,
    };

    // Fill gutter background
    renderer.fill_quad(
        renderer::Quad {
            bounds: gutter_bounds,
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
            shadow: iced::Shadow::default(),
        },
        background_color,
    );

    // Draw gutter border (separator line)
    let border_bounds = Rectangle {
        x: gutter_bounds.x + gutter_bounds.width - GUTTER_BORDER_WIDTH,
        y: gutter_bounds.y,
        width: GUTTER_BORDER_WIDTH,
        height: gutter_bounds.height,
    };

    renderer.fill_quad(
        renderer::Quad {
            bounds: border_bounds,
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
            shadow: iced::Shadow::default(),
        },
        GUTTER_BORDER_COLOR,
    );

    // Use batched text rendering with viewport culling
    let text_batches = line_numbers_cache.get_or_create_text_batches(bounds, base_padding, gutter_width, line_height);
    let text_size = Pixels(font_size);
    let font = renderer.default_font();
    let text_width = (gutter_width - GUTTER_TEXT_PADDING * 2.0).max(0.0);

    // Viewport culling: only render line numbers that are actually visible
    let viewport_top = viewport.y;
    let viewport_bottom = viewport.y + viewport.height;

    for (text_content, x, y) in text_batches.iter() {
        let text_bottom = *y + line_height;

        // Skip if text is completely outside viewport
        if text_bottom < viewport_top || *y > viewport_bottom {
            continue;
        }

        let text = PrimitiveText {
            content: text_content,
            bounds: Size::new(text_width, line_height),
            size: text_size,
            line_height: LineHeight::Absolute(Pixels(line_height)),
            font,
            horizontal_alignment: alignment::Horizontal::Right,
            vertical_alignment: alignment::Vertical::Top,
            shaping: Shaping::Basic,
        };

        renderer.fill_text(text, Point::new(*x, *y), color, *viewport);
    }
}

fn collect_visible_line_numbers_optimized(
    buffer: &CosmicBuffer,
    scroll: usize,
    visible_lines: usize,
    output: &mut Vec<usize>,
) -> Vec<usize> {
    if buffer.lines.is_empty() {
        output.push(1);
        return output.clone();
    }

    output.clear();
    output.reserve(visible_lines.saturating_add(1));

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
            output.push(line_index + 1);
            display_index += 1;

            if display_index >= max_entries {
                break;
            }
        }

        line_index += 1;
    }

    if output.is_empty() {
        output.push(current_line + 1);
    }

    output.clone()
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
