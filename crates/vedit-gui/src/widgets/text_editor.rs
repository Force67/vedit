// Allow dead code for optimization infrastructure that is built but not yet fully integrated.
// This includes: LineIndex, StreamingBuffer, various caching helpers for large file support.
#![allow(dead_code)]

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
use iced::advanced::Renderer as _;
use iced::advanced::Shell;
use iced::advanced::clipboard::Clipboard;
use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse;
use iced::advanced::renderer;
use iced::advanced::text::Highlighter as IcedHighlighter;
use iced::advanced::text::Renderer as TextRenderer;
use iced::advanced::text::highlighter;
use iced::advanced::text::{LineHeight, Shaping, Text as PrimitiveText};
use iced::advanced::widget::{Widget, tree};
use iced::alignment;
use iced::event::Event;
use iced::widget::text_editor;
pub use iced::widget::text_editor::{Action, Content};
use iced_graphics::text::Editor as GraphicsEditor;
use iced_graphics::text::cosmic_text::Buffer as CosmicBuffer;
use itoa;
use std::cell::{Cell, Ref, RefCell};
use std::collections::VecDeque;
use std::rc::Rc; // For zero-allocation integer to string conversion

use crate::style;

const DEFAULT_GUTTER_WIDTH: f32 = 56.0; // Slightly tighter
const DEFAULT_LINE_COLOR: Color = Color::from_rgba(0.7, 0.7, 0.7, 1.0);
const GUTTER_TEXT_PADDING: f32 = 10.0;
const GUTTER_BORDER_WIDTH: f32 = 1.0;
const DEBUG_DOT_RADIUS: f32 = 5.0;
const DEBUG_DOT_PADDING: f32 = 4.0;
const DEBUG_DOT_GLOW_RADIUS: f32 = 8.0; // Outer glow for breakpoints

/// Prefix-sum wrap index for O(log N) scroll-to-line mapping
#[derive(Debug, Clone)]
struct WrapIndex {
    // cumulative[i] = total visual lines up to (but not including) buffer line i
    cumulative: Vec<usize>, // len = buffer.lines.len() + 1, cumulative[0] = 0
    version: u64,           // real buffer revision
    width_hash: u64,        // invalidate if wrapping width/font changes
    total_visual_lines: usize,
}

impl WrapIndex {
    fn new() -> Self {
        Self {
            cumulative: vec![0],
            version: 0,
            width_hash: 0,
            total_visual_lines: 0,
        }
    }

    fn rebuild(&mut self, buffer: &CosmicBuffer, width_hash: u64) {
        self.cumulative.clear();
        self.cumulative.reserve(buffer.lines.len() + 1);
        self.cumulative.push(0);
        let mut running = 0usize;

        for line in &buffer.lines {
            let wraps = line.layout_opt().as_ref().map(|l| l.len()).unwrap_or(1);
            running += wraps;
            self.cumulative.push(running);
        }

        self.total_visual_lines = running;
        self.version = real_buffer_version(buffer);
        self.width_hash = width_hash;
    }

    #[inline]
    fn total_visual(&self) -> usize {
        self.total_visual_lines
    }

    // map scroll (visual line index) -> (buffer_line, wrap_offset)
    fn locate(&self, scroll: usize) -> (usize, usize) {
        // Clamp to last visual line to avoid out-of-bounds on empty or short files
        let s = scroll.min(self.total_visual_lines.saturating_sub(1));
        // Find first i with cumulative[i] > s
        let i = self.cumulative.partition_point(|&x| x <= s);
        let line = i.saturating_sub(1);
        let offset = s - self.cumulative[line];
        (line, offset)
    }

    fn is_valid(&self, buffer: &CosmicBuffer, width_hash: u64) -> bool {
        self.version == real_buffer_version(buffer) && self.width_hash == width_hash
    }
}

/// Line offset index for O(1) line access in streaming buffer
#[derive(Debug, Clone)]
struct LineIndex {
    offs: Vec<usize>, // offs[i] = byte offset of start of line i, offs.last() = content.len()
}

impl LineIndex {
    fn from_text(s: &str) -> Self {
        let bytes = s.as_bytes();
        let mut offs = Vec::with_capacity(1 + bytes.len() / 32);
        offs.push(0);
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\n' {
                offs.push(i + 1);
            }
        }
        if *offs.last().unwrap() != s.len() {
            offs.push(s.len());
        }
        Self { offs }
    }

    #[inline]
    fn len(&self) -> usize {
        self.offs.len().saturating_sub(1)
    }

    #[inline]
    fn line<'a>(&self, s: &'a str, i: usize) -> &'a str {
        let start = self.offs[i];
        let end = self.offs[i + 1];
        &s[start..end].trim_end_matches('\n')
    }
}

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

/// Optimized line number state using WrapIndex for O(log N) performance
#[derive(Debug, Clone)]
struct OptimizedLineState {
    // Wrap index for fast O(log N) scroll-to-line mapping
    wrap_index: WrapIndex,
    cached_scroll: usize,                 // Last processed scroll position
    last_update_time: std::time::Instant, // Throttle scroll processing
    // Fast path optimization
    last_known_position: Option<(usize, usize)>, // Cached (buffer_line, wrap_offset)
    buffer_width_hash: u64,                      // Track changes to wrapping width
}

impl OptimizedLineState {
    fn new() -> Self {
        Self {
            wrap_index: WrapIndex::new(),
            cached_scroll: 0,
            last_update_time: std::time::Instant::now(),
            last_known_position: None,
            buffer_width_hash: 0,
        }
    }

    fn is_valid(&self, buffer: &CosmicBuffer, current_scroll: usize) -> bool {
        let now = std::time::Instant::now();
        let time_since_last = now.duration_since(self.last_update_time).as_millis() as u64;

        // Only throttle if content hasn't changed and scroll is small
        let scroll_delta = if self.cached_scroll > current_scroll {
            self.cached_scroll - current_scroll
        } else {
            current_scroll - self.cached_scroll
        };

        // 1) Never accept fast path if layout/content invalid
        let current_width_hash = compute_width_hash(buffer);
        if !self.wrap_index.is_valid(buffer, current_width_hash) {
            return false;
        }
        // 2) Otherwise, allow smooth-scroll throttle
        if scroll_delta <= 2 && time_since_last < 4 {
            return true;
        }
        true
    }

    fn update(&mut self, buffer: &CosmicBuffer, scroll: usize) {
        self.cached_scroll = scroll;
        self.last_update_time = std::time::Instant::now();
        self.buffer_width_hash = compute_width_hash(buffer);

        // Rebuild wrap index if needed
        if !self.wrap_index.is_valid(buffer, self.buffer_width_hash) {
            self.wrap_index.rebuild(buffer, self.buffer_width_hash);
        }

        // Update cached position using O(log N) lookup
        self.last_known_position = Some(self.wrap_index.locate(scroll));
    }

    fn get_visible_lines(
        &self,
        start_scroll: usize,
        visible_lines: usize,
        _total_lines: usize,
    ) -> Vec<usize> {
        let (start_buffer_line, start_wrap_offset) = self
            .last_known_position
            .unwrap_or_else(|| self.wrap_index.locate(start_scroll));

        self.compute_visible_lines_optimized(start_buffer_line, start_wrap_offset, visible_lines)
    }

    fn compute_visible_lines_optimized(
        &self,
        start_buffer_line: usize,
        start_wrap_offset: usize,
        visible_lines: usize,
    ) -> Vec<usize> {
        let mut result = Vec::with_capacity(visible_lines.saturating_add(1));
        let mut current_buffer_line = start_buffer_line;
        let mut current_wrap_offset = start_wrap_offset;
        let mut display_index = 0;

        while display_index < visible_lines
            && current_buffer_line < self.wrap_index.cumulative.len().saturating_sub(1)
        {
            let wraps = if current_buffer_line + 1 < self.wrap_index.cumulative.len() {
                self.wrap_index.cumulative[current_buffer_line + 1]
                    - self.wrap_index.cumulative[current_buffer_line]
            } else {
                1
            };

            for _ in current_wrap_offset..wraps {
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

// Legacy IncrementalLineState for backward compatibility (will be removed)
#[derive(Debug, Clone)]
struct IncrementalLineState {
    optimized: OptimizedLineState,
}

impl IncrementalLineState {
    fn new() -> Self {
        Self {
            optimized: OptimizedLineState::new(),
        }
    }

    fn is_valid(&self, buffer: &CosmicBuffer, current_scroll: usize) -> bool {
        self.optimized.is_valid(buffer, current_scroll)
    }

    fn update(&mut self, buffer: &CosmicBuffer, scroll: usize) {
        self.optimized.update(buffer, scroll);
    }

    fn get_visible_lines(
        &self,
        start_scroll: usize,
        visible_lines: usize,
        total_lines: usize,
    ) -> Vec<usize> {
        self.optimized
            .get_visible_lines(start_scroll, visible_lines, total_lines)
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

    fn is_valid(
        &self,
        scroll: usize,
        visible_lines: usize,
        total_lines: usize,
        font_size: f32,
        line_height: f32,
    ) -> bool {
        self.scroll == scroll
            && self.visible_lines == visible_lines
            && self.total_lines == total_lines
            && (self.font_size - font_size).abs() < f32::EPSILON
            && (self.line_height - line_height).abs() < f32::EPSILON
    }

    fn update(
        &mut self,
        numbers: Vec<usize>,
        scroll: usize,
        visible_lines: usize,
        total_lines: usize,
        font_size: f32,
        line_height: f32,
    ) {
        self.numbers = numbers;
        self.scroll = scroll;
        self.visible_lines = visible_lines;
        self.total_lines = total_lines;
        self.font_size = font_size;
        self.line_height = line_height;
        self.batch_valid = false; // Invalidate cached batches
    }

    fn get_or_create_text_batches(
        &mut self,
        bounds: Rectangle,
        base_padding: &Padding,
        gutter_width: f32,
        line_height: f32,
    ) -> &[(String, f32, f32)] {
        // Always regenerate if bounds changed significantly (window resize, etc.)
        let should_regenerate = !self.batch_valid
            || self.cached_text_batches.is_empty()
            || self.cached_text_batches.len() != self.numbers.len();

        if should_regenerate {
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
    buffer_version: u64,                  // Track buffer changes
    width_hash: u64,                      // Track layout-affecting changes
    current_scroll: usize,                // Cache current scroll position
    last_render_time: std::time::Instant, // Throttle updates
    should_stream: bool,                  // Cached streaming decision
    wrap_index: WrapIndex,                // Cached wrap index for O(1) access
}

impl CachedLineMetrics {
    fn new() -> Self {
        Self {
            line_height: 0.0,
            font_size: 0.0,
            visible_lines: 0,
            total_visual_lines: 0,
            buffer_version: 0,
            width_hash: 0,
            current_scroll: 0,
            last_render_time: std::time::Instant::now(),
            should_stream: false,
            wrap_index: WrapIndex::new(),
        }
    }

    fn needs_update(&self, buffer: &CosmicBuffer, font_size: f32, scroll: usize) -> bool {
        let now = std::time::Instant::now();
        // Only throttle if content hasn't changed and scroll is small
        let time_since_last = now.duration_since(self.last_render_time).as_millis();
        let small_scroll_change = if self.current_scroll > scroll {
            self.current_scroll - scroll <= 1 // More sensitive for smooth scrolling
        } else {
            scroll - self.current_scroll <= 1
        };

        let current_width_hash = compute_width_hash(buffer);

        // Always update if scroll changed significantly or content/layout changed
        if !small_scroll_change
            || self.buffer_version != get_buffer_version(buffer)
            || self.width_hash != current_width_hash
            || (self.font_size - font_size).abs() > f32::EPSILON
        {
            return true;
        }

        // Only throttle for very small changes - this eliminates per-frame O(N) scans
        time_since_last >= 8 // ~120Hz max, but typically much less frequent updates
    }

    fn is_valid(&self, buffer: &CosmicBuffer, font_size: f32) -> bool {
        let current_width_hash = compute_width_hash(buffer);
        (self.font_size - font_size).abs() < f32::EPSILON
            && self.buffer_version == get_buffer_version(buffer)
            && self.width_hash == current_width_hash
    }

    fn update(&mut self, buffer: &CosmicBuffer, font_size: f32, scroll: usize) {
        self.line_height = buffer.metrics().line_height.max(1.0);
        self.font_size = font_size;
        self.visible_lines = calculate_visible_lines(buffer, None);

        // Use a cached wrap index to avoid O(N) each frame
        let width_hash = compute_width_hash(buffer);
        if self.width_hash != width_hash || self.buffer_version != get_buffer_version(buffer) {
            self.wrap_index.rebuild(buffer, width_hash);
            self.total_visual_lines = self.wrap_index.total_visual();

            // Cache streaming decision: use streaming for files with more than 1000 logical lines OR 10,000 visual lines
            let total_logical_lines = buffer.lines.len();
            self.should_stream = total_logical_lines > 1000 || self.total_visual_lines > 10_000;

            self.width_hash = width_hash;
        }

        self.buffer_version = get_buffer_version(buffer);
        self.current_scroll = scroll;
        self.last_render_time = std::time::Instant::now();
    }
}

// Real buffer version tracking using a global counter
static mut GLOBAL_BUFFER_VERSION: u64 = 1;

// Simple versioning for buffer changes (using address as heuristic)
fn get_buffer_version(_buffer: &CosmicBuffer) -> u64 {
    // In a real implementation, you'd want proper version tracking
    // For now, we use the buffer's memory address as a heuristic
    _buffer as *const _ as u64
}

// Real buffer version tracking function
fn real_buffer_version(_buffer: &CosmicBuffer) -> u64 {
    // Use global edit epoch for proper invalidation
    unsafe { GLOBAL_BUFFER_VERSION }
}

// Function to increment global buffer version when content changes
fn increment_buffer_version() -> u64 {
    unsafe {
        GLOBAL_BUFFER_VERSION += 1;
        GLOBAL_BUFFER_VERSION
    }
}

// Function to get a width hash that captures layout-affecting properties
fn compute_width_hash(buffer: &CosmicBuffer) -> u64 {
    let metrics = buffer.metrics();
    // Hash font size, line height, and other layout-affecting properties
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    metrics.font_size.to_bits().hash(&mut hasher);
    metrics.line_height.to_bits().hash(&mut hasher);
    // Add any other layout-affecting properties here
    hasher.finish()
}

/// Get the scroll line from a buffer's scroll position.
/// In cosmic_text 0.15+, Scroll is a struct with a `line` field.
#[inline]
fn get_scroll_line(buffer: &CosmicBuffer) -> usize {
    buffer.scroll().line
}

/// Calculate visible lines based on buffer metrics.
/// This replaces the removed `visible_lines()` method from cosmic_text.
#[inline]
fn calculate_visible_lines(buffer: &CosmicBuffer, viewport_height: Option<f32>) -> usize {
    let metrics = buffer.metrics();
    let line_height = metrics.line_height.max(1.0);
    // Use buffer size if no viewport provided, otherwise use viewport
    let height = viewport_height.unwrap_or_else(|| {
        buffer.size().1.unwrap_or(600.0) // Default viewport height
    });
    (height / line_height).ceil() as usize
}

/// Red dot marker for debugging integration
#[derive(Debug, Clone)]
pub struct DebugDot {
    pub line_number: usize,
    pub enabled: bool,
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
    search_highlight_line: Option<usize>,
    indent_guides: Option<Color>,
    gutter_background: Option<Color>,
    show_minimap: bool,
    font_size: Option<Pixels>,
    debug_dots: Vec<DebugDot>,
    on_gutter_click: Option<Rc<dyn Fn(usize) -> Message>>,
    hover_line: Option<usize>,
    cached_line_numbers: Rc<RefCell<CachedLineNumbers>>,
    cached_line_metrics: Rc<RefCell<CachedLineMetrics>>,
    cached_scroll_metrics: Rc<RefCell<CachedScrollMetrics>>,
    incremental_line_state: Rc<RefCell<IncrementalLineState>>,
    streaming_buffer: Rc<RefCell<StreamingBuffer>>,
}

impl<'a, Message> TextEditor<'a, Message, highlighter::PlainText> {
    pub fn new(content: &'a Content) -> Self {
        let base_padding = Padding::new(5.0);
        let gutter_width = DEFAULT_GUTTER_WIDTH;
        let mut inner = text_editor::TextEditor::new(content);
        let effective = add_gutter(base_padding, gutter_width);
        // Set a large width to make the editor fill available space
        inner = inner.padding(effective).width(10000.0);
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
            search_highlight_line: None,
            indent_guides: None,
            gutter_background: Some(style::GUTTER_BG),
            show_minimap: false,
            font_size: None,
            debug_dots: Vec::new(),
            on_gutter_click: None,
            hover_line: None,
            cached_line_numbers: Rc::new(RefCell::new(CachedLineNumbers::new())),
            cached_line_metrics: Rc::new(RefCell::new(CachedLineMetrics::new())),
            cached_scroll_metrics: Rc::new(RefCell::new(CachedScrollMetrics::new())),
            incremental_line_state: Rc::new(RefCell::new(IncrementalLineState::new())),
            streaming_buffer: Rc::new(RefCell::new(StreamingBuffer::new())),
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
            inner: self.inner.highlight_with(settings, to_format),
            content: self.content,
            base_padding: self.base_padding,
            gutter_width: self.gutter_width,
            line_color: self.line_color,
            pointer_correction: Rc::clone(&self.pointer_correction),
            current_line_highlight: self.current_line_highlight,
            search_highlight_line: self.search_highlight_line,
            indent_guides: self.indent_guides,
            gutter_background: self.gutter_background,
            show_minimap: self.show_minimap,
            font_size: self.font_size,
            debug_dots: self.debug_dots.clone(),
            on_gutter_click: self.on_gutter_click.clone(),
            hover_line: self.hover_line,
            cached_line_numbers: Rc::clone(&self.cached_line_numbers),
            cached_line_metrics: Rc::clone(&self.cached_line_metrics),
            cached_scroll_metrics: Rc::clone(&self.cached_scroll_metrics),
            incremental_line_state: Rc::clone(&self.incremental_line_state),
            streaming_buffer: Rc::clone(&self.streaming_buffer),
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
            // Increment buffer version for content-modifying actions to invalidate caches
            if matches!(action, Action::Edit(_)) {
                let _ = increment_buffer_version();
            }
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
        self.pointer_correction.set(pointer_correction_value(
            self.base_padding,
            self.gutter_width,
        ));
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

    pub fn search_highlight_line(mut self, line_number: Option<usize>) -> Self {
        self.search_highlight_line = line_number;
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

    pub fn debug_dots(mut self, dots: Vec<DebugDot>) -> Self {
        self.debug_dots = dots;
        self
    }

    pub fn add_debug_dot(mut self, line_number: usize) -> Self {
        self.debug_dots.push(DebugDot {
            line_number,
            enabled: true,
        });
        self
    }

    pub fn remove_debug_dot(mut self, line_number: usize) -> Self {
        self.debug_dots.retain(|dot| dot.line_number != line_number);
        self
    }

    pub fn clear_debug_dots(mut self) -> Self {
        self.debug_dots.clear();
        self
    }

    pub fn on_gutter_click<F>(mut self, f: F) -> Self
    where
        F: Fn(usize) -> Message + 'a + 'static,
    {
        self.on_gutter_click = Some(Rc::new(f));
        self
    }

    /// Get cached scroll metrics, updating cache if needed
    pub fn cached_scroll_metrics(&self) -> ScrollMetrics {
        let editor_ref = borrow_editor(self.content);
        let buffer = editor_ref.buffer();
        let current_scroll = get_scroll_line(buffer);

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
        let incremental_state = self.incremental_line_state.borrow_mut();
        drop(incremental_state); // Force refresh by dropping and recreating
        *self.incremental_line_state.borrow_mut() = IncrementalLineState::new();
    }

    /// Get line number from cursor position in the gutter
    fn get_line_number_from_position(&self, position: Point, bounds: Rectangle) -> Option<usize> {
        let _editor_ref = borrow_editor(self.content);
        let buffer = _editor_ref.buffer();
        let line_height = buffer.metrics().line_height.max(1.0);
        let scroll = get_scroll_line(buffer);
        let start_y = bounds.y + self.base_padding.top;

        // Calculate which line was clicked based on y position
        let relative_y = position.y - start_y;
        if relative_y < 0.0 {
            return None;
        }

        // Calculate line number considering scroll
        let clicked_line_float = (relative_y / line_height) + scroll as f32;
        let line_number = clicked_line_float as usize;

        // Get the actual line number from the incremental state for accuracy
        let incremental_state = self.incremental_line_state.borrow();
        let visible_lines = calculate_visible_lines(buffer, None);
        let total_lines = incremental_state.optimized.wrap_index.total_visual();

        let visible_line_numbers =
            incremental_state.get_visible_lines(scroll, visible_lines, total_lines);

        if line_number < visible_line_numbers.len() {
            Some(visible_line_numbers[line_number])
        } else {
            // Fallback calculation
            Some(line_number + 1)
        }
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
        // Return default fill size for the text editor
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(
        &mut self,
        tree: &mut tree::Tree,
        renderer: &IcedRenderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        // Force the widget to fill all available space
        let max = limits.max();
        let min = limits.min();

        // Use max dimensions, falling back to min or reasonable defaults
        let width = if max.width.is_finite() {
            max.width
        } else {
            min.width.max(800.0)
        };
        let height = if max.height.is_finite() {
            max.height
        } else {
            min.height.max(600.0)
        };

        // Still run inner layout to set up tree state, but ignore its size
        let _inner_node = self.inner.layout(tree, renderer, limits);

        // Return a node that fills the space - no children, inner will draw at our bounds
        layout::Node::new(Size::new(width, height))
    }

    fn update(
        &mut self,
        tree: &mut tree::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &IcedRenderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        // Handle gutter click events
        if let (Some(gutter_click_handler), Some(cursor_pos)) =
            (&self.on_gutter_click, cursor.position_over(layout.bounds()))
        {
            let gutter_right = layout.bounds().x + self.base_padding.left + self.gutter_width;

            // Check if click is in gutter area
            if cursor_pos.x < gutter_right {
                if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
                    if let Some(line_number) =
                        self.get_line_number_from_position(cursor_pos, layout.bounds())
                    {
                        shell.publish(gutter_click_handler(line_number));
                        return;
                    }
                }
            }
        }

        // Delegate to inner text editor for other events
        self.inner.update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
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

        // Draw search highlight if active
        if let Some(highlight_line) = self.search_highlight_line {
            draw_search_highlight_static(
                renderer,
                bounds,
                viewport,
                highlight_line,
                self.content,
                self.base_padding,
                self.gutter_width,
                self.font_size,
            );
        }

        draw_line_numbers_optimized_with_background(
            renderer,
            bounds,
            viewport,
            &self.base_padding,
            self.gutter_width,
            self.line_color,
            self.gutter_background.unwrap_or(style::GUTTER_BG),
            self.content,
            self.font_size.map(|p| p.0),
            &self.cached_line_numbers,
            &self.cached_line_metrics,
            &self.incremental_line_state,
        );

        // Draw debug dots in the gutter area
        draw_debug_dots(
            renderer,
            bounds,
            viewport,
            &self.base_padding,
            self.gutter_width,
            &self.debug_dots,
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
        if let Some(position) = cursor.position_over(layout.bounds()) {
            // Check if cursor is in gutter area
            let gutter_right = layout.bounds().x + self.base_padding.left + self.gutter_width;

            if position.x < gutter_right {
                // Cursor is in gutter area - show pointer cursor for clickable debug dots
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

fn draw_search_highlight_static(
    renderer: &mut IcedRenderer,
    bounds: Rectangle,
    viewport: &Rectangle,
    highlight_line: usize,
    content: &Content,
    base_padding: Padding,
    gutter_width: f32,
    font_size: Option<Pixels>,
) {
    let _editor_ref = borrow_editor(content);
    let buffer = _editor_ref.buffer();
    let _font_size = font_size.map(|p| p.0).unwrap_or(buffer.metrics().font_size);
    let line_height = buffer.metrics().line_height.max(1.0);
    let scroll = get_scroll_line(buffer) as f32;

    // Calculate line position using the same logic as line numbers
    let start_y = bounds.y + base_padding.top;
    let line_y = (highlight_line as f32 - scroll) * line_height;
    let highlight_y = start_y + line_y - line_height; // Subtract one line height to fix offset

    // Only draw if line is visible in viewport
    if highlight_y + line_height >= viewport.y && highlight_y <= viewport.y + viewport.height {
        let highlight_rect = Rectangle {
            x: bounds.x + base_padding.left + gutter_width - 1.0, // Start near gutter with small padding
            y: highlight_y, // Use the same calculation as line numbers
            width: bounds.width - (base_padding.left + base_padding.right) - gutter_width + 2.0, // Extend past gutter with small padding
            height: line_height, // Use full line height
        };

        // Create a semi-transparent yellow highlight
        let highlight_color = Color::from_rgba(1.0, 0.9, 0.3, 0.3); // Yellow with transparency

        // Draw the highlight rectangle
        renderer.fill_quad(
            renderer::Quad {
                bounds: highlight_rect,
                border: iced::border::Border {
                    radius: 2.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                shadow: iced::Shadow::default(),
                snap: true,
            },
            highlight_color,
        );
    }
}

impl<'a, Message, H> From<TextEditor<'a, Message, H>>
    for Element<'a, Message, IcedTheme, IcedRenderer>
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

// Streaming buffer for truly large files - only loads visible lines
#[derive(Debug)]
struct StreamingBuffer {
    // The complete file content (as bytes or string) - this stays on disk
    file_content: Option<String>,
    // Line index for O(1) line access
    line_index: Option<LineIndex>,
    // Sliding window of loaded line indices (no String allocations needed for gutter)
    loaded: std::collections::VecDeque<usize>,
    // Starting line number in the file for loaded[0]
    loaded_start: usize,
    // Total lines in the file
    total_lines: usize,
    // Maximum lines to keep in memory
    max_loaded: usize,
    // Pre-allocated string pool for line numbers
    string_pool: Vec<String>,
    // Render batch for visible lines only
    render_batch: VecDeque<(usize, String)>,
    // Last viewport for cache invalidation
    last_viewport: Option<Rectangle>,
}

impl StreamingBuffer {
    fn new() -> Self {
        let mut string_pool = Vec::with_capacity(10000);
        // Pre-allocate strings for common line numbers (1-9999)
        for i in 1..=10000 {
            string_pool.push(i.to_string());
        }

        Self {
            file_content: None,
            line_index: None,
            loaded: std::collections::VecDeque::with_capacity(200), // Start with small viewport
            loaded_start: 0,
            total_lines: 0,
            max_loaded: 500, // Only keep 500 lines in memory
            string_pool,
            render_batch: VecDeque::with_capacity(100), // Small render batch
            last_viewport: None,
        }
    }

    // New method: Initialize from cosmic-text buffer directly
    fn initialize_from_cosmic_buffer(&mut self, buffer: &CosmicBuffer) {
        self.total_lines = buffer.lines.len();

        // Extract text content from cosmic-text buffer once
        let mut text = String::new();
        for line in buffer.lines.iter() {
            // Try to get line text - cosmic-text lines don't have a simple text() method
            // For now, use the Debug representation as fallback
            text.push_str(&format!("{:?}\n", line));
        }

        self.file_content = Some(text);

        // Build line index immediately to avoid the None issue
        if let Some(ref content) = self.file_content {
            self.line_index = Some(LineIndex::from_text(content));
        }

        self.loaded.clear();
        self.loaded_start = 0;
    }

    fn load_file_content(&mut self, content: &str) {
        self.file_content = Some(content.to_string());
        self.line_index = Some(LineIndex::from_text(content));
        self.total_lines = self.line_index.as_ref().map(|idx| idx.len()).unwrap_or(0);
        self.loaded.clear();
        self.loaded_start = 0;
    }

    fn ensure_lines_loaded(
        &mut self,
        target_buffer_line: usize,
        approx_visible_buffer_lines: usize,
    ) {
        if self.file_content.is_none() || self.line_index.is_none() {
            return;
        }

        // Calculate the range we need to have loaded (in buffer lines, not visual lines)
        let buffer_above = 50; // Load 50 lines above viewport
        let buffer_below = 50; // Load 50 lines below viewport
        let needed_start = target_buffer_line.saturating_sub(buffer_above);
        let total_lines = self.line_index.as_ref().map(|idx| idx.len()).unwrap_or(0);
        let needed_end =
            (target_buffer_line + approx_visible_buffer_lines + buffer_below).min(total_lines);

        // Initial fill: populate the window if empty
        if self.loaded.is_empty() {
            self.loaded_start = needed_start;
            let end = needed_end.min(total_lines);
            self.loaded.extend(self.loaded_start..end);
            return;
        }

        // Slide down: add lines at the back, remove from front
        while self.loaded_start + self.loaded.len() < needed_end {
            let next = self.loaded_start + self.loaded.len();
            if next >= total_lines {
                break;
            }
            self.loaded.push_back(next);
            if self.loaded.len() > self.max_loaded {
                self.loaded.pop_front();
                self.loaded_start += 1;
            }
        }

        // Slide up: add lines at the front, remove from back
        while needed_start < self.loaded_start {
            let prev = self.loaded_start - 1;
            self.loaded.push_front(prev);
            self.loaded_start -= 1;
            if self.loaded.len() > self.max_loaded {
                self.loaded.pop_back();
            }
        }
    }

    #[inline]
    fn total_lines(&self) -> usize {
        self.line_index.as_ref().map(|li| li.len()).unwrap_or(0)
    }

    fn get_loaded_line(&self, _line_number: usize) -> Option<&str> {
        // Not used for gutter rendering - gutter uses the computed numbers vector
        // This method can be used for actual text content streaming if needed
        None
    }

    fn prepare_viewport_batch(
        &mut self,
        start_line: usize,
        visible_lines: usize,
        viewport: &Rectangle,
        bounds: Rectangle,
        line_height: f32,
    ) {
        let viewport_top = viewport.y;
        let viewport_bottom = viewport.y + viewport.height;
        let start_y = bounds.y + 10.0; // Approximate base padding top

        // Clear previous batch
        self.render_batch.clear();

        // Calculate which lines are actually visible
        let _start_visible_line = ((viewport_top - start_y) / line_height).floor() as usize;
        let _end_visible_line = ((viewport_bottom - start_y) / line_height).ceil() as usize + 1;

        // Ensure we have the needed lines loaded
        self.ensure_lines_loaded(start_line, visible_lines);

        // Prepare batch with only visible lines that we have loaded
        for line_offset in 0..visible_lines {
            let line_num = start_line + line_offset;

            // Skip if we don't have this line loaded
            if let Some(_line_content) = self.get_loaded_line(line_num) {
                let y = start_y + line_offset as f32 * line_height;
                let text_bottom = y + line_height;

                // Skip if not in viewport (extra culling)
                if text_bottom < viewport_top || y > viewport_bottom {
                    continue;
                }

                let line_str = if line_num >= 1 && line_num <= 10000 {
                    self.string_pool[line_num - 1].clone()
                } else {
                    line_num.to_string()
                };

                self.render_batch.push_back((line_num, line_str));
            }
        }

        self.last_viewport = Some(*viewport);
    }

    fn render_batch(
        &mut self,
        renderer: &mut IcedRenderer,
        bounds: Rectangle,
        base_padding: &Padding,
        gutter_width: f32,
        color: Color,
        font_size: f32,
        line_height: f32,
        viewport: &Rectangle,
        start_line: usize,
    ) {
        let text_size = Pixels(font_size);
        let font = renderer.default_font();
        let text_width = (gutter_width - GUTTER_TEXT_PADDING * 2.0).max(0.0);
        let gutter_right = bounds.x + base_padding.left + gutter_width;
        let start_y = bounds.y + base_padding.top;

        // Batch render all visible lines
        while let Some((line_number, line_str)) = self.render_batch.pop_front() {
            let index = line_number.saturating_sub(start_line); // Convert to 0-based index
            let y = start_y + index as f32 * line_height;

            // Create text primitive with owned content
            let text = PrimitiveText {
                content: line_str, // Owned String
                bounds: Size::new(text_width, line_height),
                size: text_size,
                line_height: LineHeight::Absolute(Pixels(line_height)),
                font,
                align_x: alignment::Horizontal::Right.into(),
                align_y: alignment::Vertical::Top,
                shaping: Shaping::Basic,
                wrapping: iced::advanced::text::Wrapping::None,
            };

            let x = (gutter_right - text_width - GUTTER_TEXT_PADDING).max(bounds.x);
            renderer.fill_text(text, Point::new(x, y), color, *viewport);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScrollMetrics {
    pub scroll: usize,
    pub visible_lines: usize,
    pub total_visual_lines: usize,
}

impl ScrollMetrics {
    pub fn max_scroll(&self) -> usize {
        self.total_visual_lines.saturating_sub(self.visible_lines)
    }
}

/// Cached scroll metrics to avoid repeated buffer queries
#[derive(Debug, Clone)]
struct CachedScrollMetrics {
    metrics: ScrollMetrics,
    buffer_version: u64,
    width_hash: u64,
    last_scroll: Option<usize>,
    wrap_index: WrapIndex, // Embedded wrap index for O(1) total_visual_lines
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
            width_hash: 0,
            last_scroll: None,
            wrap_index: WrapIndex::new(),
        }
    }

    fn is_valid(&self, buffer: &CosmicBuffer, current_scroll: usize) -> bool {
        let current_width_hash = compute_width_hash(buffer);
        self.buffer_version == get_buffer_version(buffer)
            && self.width_hash == current_width_hash
            && self
                .last_scroll
                .map_or(false, |last| last == current_scroll)
    }

    fn update(&mut self, buffer: &CosmicBuffer) {
        let visible_lines = calculate_visible_lines(buffer, None);
        let scroll = get_scroll_line(buffer);
        let width_hash = compute_width_hash(buffer);

        // Rebuild wrap index if needed - this is the only O(N) operation
        if !self.wrap_index.is_valid(buffer, width_hash) {
            self.wrap_index.rebuild(buffer, width_hash);
        }

        self.metrics = ScrollMetrics {
            scroll,
            visible_lines,
            total_visual_lines: self.wrap_index.total_visual(),
        };
        self.buffer_version = get_buffer_version(buffer);
        self.width_hash = width_hash;
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
    let current_scroll = get_scroll_line(buffer);

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
    let delta = delta.clamp(i32::MIN as isize, i32::MAX as isize) as i32;

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
    let visible_lines = calculate_visible_lines(buffer, None);
    let scroll = get_scroll_line(buffer);
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
            snap: true,
        },
        style::GUTTER_BG,
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
            snap: true,
        },
        style::GUTTER_BORDER,
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
        let text = PrimitiveText {
            content: line_number.to_string(), // Owned String
            bounds: Size::new(text_width, line_height),
            size: text_size,
            line_height: LineHeight::Absolute(Pixels(line_height)),
            font,
            align_x: alignment::Horizontal::Right.into(),
            align_y: alignment::Vertical::Top,
            shaping: Shaping::Basic,
            wrapping: iced::advanced::text::Wrapping::None,
        };

        renderer.fill_text(text, Point::new(start_x, y), color, *viewport);
    }
}

fn render_gutter_numbers(
    renderer: &mut IcedRenderer,
    bounds: Rectangle,
    viewport: &Rectangle,
    base_padding: &Padding,
    gutter_width: f32,
    color: Color,
    font_size: f32,
    line_height: f32,
    numbers: &[usize],
) {
    let text_width = (gutter_width - GUTTER_TEXT_PADDING * 2.0).max(0.0);
    let gutter_right = bounds.x + base_padding.left + gutter_width;
    let start_y = bounds.y + base_padding.top;
    let start_x = (gutter_right - text_width - GUTTER_TEXT_PADDING).max(bounds.x);
    let font = renderer.default_font();

    // Optional: cull with the viewport
    let top = viewport.y;
    let bottom = viewport.y + viewport.height;

    // Zero-allocation formatting
    let mut itoa_buf = itoa::Buffer::new();

    for (i, &n) in numbers.iter().enumerate() {
        let y = start_y + (i as f32) * line_height;
        if y + line_height < top || y > bottom {
            continue;
        }
        let s = itoa_buf.format(n);
        let text = PrimitiveText {
            content: s.to_string(), // Convert to owned String
            bounds: Size::new(text_width, line_height),
            size: Pixels(font_size),
            line_height: LineHeight::Absolute(Pixels(line_height)),
            font,
            align_x: alignment::Horizontal::Right.into(),
            align_y: alignment::Vertical::Top,
            shaping: Shaping::Basic,
            wrapping: iced::advanced::text::Wrapping::None,
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
    let scroll = get_scroll_line(buffer);

    // Fast path: use cached values during smooth scrolling to avoid expensive buffer queries
    let mut metrics_cache = cached_line_metrics.borrow_mut();
    let mut incremental_state = incremental_line_state.borrow_mut();

    // Only update caches if really necessary
    let should_update_metrics = metrics_cache.needs_update(buffer, font_size, scroll);
    let should_update_incremental = !incremental_state.is_valid(buffer, scroll);

    if should_update_metrics {
        metrics_cache.update(buffer, font_size, scroll);
    }

    let line_height = metrics_cache.line_height;
    let visible_lines = metrics_cache.visible_lines;
    let total_lines = metrics_cache.total_visual_lines;

    if should_update_incremental {
        incremental_state.update(buffer, scroll);
    }

    // Get line numbers using the incremental approach
    let numbers = incremental_state.get_visible_lines(scroll, visible_lines, total_lines);

    // Update the traditional cache as well for compatibility
    let mut line_numbers_cache = cached_line_numbers.borrow_mut();
    if !line_numbers_cache.is_valid(scroll, visible_lines, total_lines, font_size, line_height) {
        line_numbers_cache.update(
            numbers.clone(),
            scroll,
            visible_lines,
            total_lines,
            font_size,
            line_height,
        );
    }

    // Use the cached numbers for rendering (avoids clone)
    let numbers_for_render = &line_numbers_cache.numbers;

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
            snap: true,
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
            snap: true,
        },
        style::GUTTER_BORDER,
    );

    // Always render gutter numbers from the cached numbers vector
    // This eliminates the streaming branch that was causing the issue
    render_gutter_numbers(
        renderer,
        bounds,
        viewport,
        base_padding,
        gutter_width,
        color,
        font_size,
        line_height,
        numbers_for_render,
    );
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
            line.layout_opt()
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

fn extract_text_from_content(content: &Content) -> String {
    let editor_ref = borrow_editor(content);
    let buffer = editor_ref.buffer();

    // Try to extract text from cosmic-text buffer
    // This is a bit hacky but should work for our purposes
    let mut text = String::new();
    for line in buffer.lines.iter() {
        // BufferLine doesn't have a simple text() method
        // For now, use Debug representation as fallback
        text.push_str(&format!("{:?}\n", line));
    }
    text
}

fn draw_debug_dots(
    renderer: &mut IcedRenderer,
    bounds: Rectangle,
    _viewport: &Rectangle,
    base_padding: &Padding,
    gutter_width: f32,
    debug_dots: &[DebugDot],
    content: &Content,
    font_size_override: Option<f32>,
) {
    if debug_dots.is_empty() {
        return;
    }

    let _editor_ref = borrow_editor(content);
    let buffer = _editor_ref.buffer();
    let _font_size = font_size_override.unwrap_or(buffer.metrics().font_size);
    let line_height = buffer.metrics().line_height.max(1.0);
    let scroll = get_scroll_line(buffer);

    // Calculate positions
    let start_y = bounds.y + base_padding.top;
    let gutter_right = bounds.x + base_padding.left + gutter_width;

    // Position dots to the right of line numbers, with some padding
    let dot_x = gutter_right - DEBUG_DOT_PADDING - DEBUG_DOT_RADIUS;

    // Render debug dots with proper viewport bounds checking
    let buffer_top = bounds.y + base_padding.top;
    let buffer_bottom = bounds.y + bounds.height - base_padding.bottom;

    for debug_dot in debug_dots.iter().filter(|dot| dot.enabled) {
        let line_number = debug_dot.line_number;

        // Calculate the y position for this line
        let line_y = (line_number as f32 - scroll as f32) * line_height;
        let dot_y = start_y + line_y - line_height + (line_height / 2.0); // Center in the line

        // Aggressive culling: only render if dot is within buffer bounds
        let dot_top = dot_y - DEBUG_DOT_RADIUS;
        let dot_bottom = dot_y + DEBUG_DOT_RADIUS;

        if dot_top >= buffer_top && dot_bottom <= buffer_bottom {
            // Draw outer glow first (semi-transparent larger circle)
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: dot_x - DEBUG_DOT_GLOW_RADIUS,
                        y: dot_y - DEBUG_DOT_GLOW_RADIUS,
                        width: DEBUG_DOT_GLOW_RADIUS * 2.0,
                        height: DEBUG_DOT_GLOW_RADIUS * 2.0,
                    },
                    border: iced::Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: DEBUG_DOT_GLOW_RADIUS.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
                style::BREAKPOINT_GLOW,
            );

            // Draw the solid red dot on top
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: dot_x - DEBUG_DOT_RADIUS,
                        y: dot_y - DEBUG_DOT_RADIUS,
                        width: DEBUG_DOT_RADIUS * 2.0,
                        height: DEBUG_DOT_RADIUS * 2.0,
                    },
                    border: iced::Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: DEBUG_DOT_RADIUS.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
                style::BREAKPOINT_COLOR,
            );
        }
    }
}
