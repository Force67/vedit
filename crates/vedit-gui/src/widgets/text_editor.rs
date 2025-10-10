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
use crate::app::REFRESH_RATE_CONFIG;
use iced::Theme as IcedTheme;
use iced::advanced::text::{LineHeight, Shaping, Text as PrimitiveText};
use iced::advanced::text::highlighter;
use iced::advanced::text::Highlighter as IcedHighlighter;
use std::sync::Arc;
use std::collections::VecDeque;
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
    // Ultra-fast text rendering cache
    precomputed_text_elements: std::collections::HashMap<usize, PrimitiveText<'static, iced::Font>>, // line_number -> cached text
    text_cache_bounds: Option<Rectangle>, // Bounds for which text cache is valid
    text_cache_font_size: Option<f32>, // Font size for which text cache is valid
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
    // Ultra-fast path optimizations
    precomputed_scrolls: std::collections::HashMap<usize, (usize, usize)>, // scroll -> (start_line, start_offset)
    last_scroll_direction: i8, // -1 for up, 1 for down, 0 for none
    consecutive_smooth_scrolls: u32, // Track smooth scrolling for optimization
    // Adaptive performance optimization
    adaptive_throttle_ms: u64, // Adaptive throttle based on performance
    frame_time_samples: [u64; 8], // Recent frame times for adaptive throttling
    frame_time_index: usize, // Current index in frame_time_samples
    high_performance_mode: bool, // Enable/disable optimizations
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
            precomputed_scrolls: std::collections::HashMap::new(),
            last_scroll_direction: 0,
            consecutive_smooth_scrolls: 0,
            adaptive_throttle_ms: 2, // Start with 500Hz (2ms)
            frame_time_samples: [2; 8], // Initialize with 2ms samples
            frame_time_index: 0,
            high_performance_mode: true,
        }
    }

    fn is_valid(&self, buffer: &CosmicBuffer, current_scroll: usize) -> bool {
        let now = std::time::Instant::now();
        let time_since_last = now.duration_since(self.last_update_time).as_millis() as u64;

        // Always update if content changed or scroll changed significantly
        let scroll_delta = if self.cached_scroll > current_scroll {
            self.cached_scroll - current_scroll
        } else {
            current_scroll - self.cached_scroll
        };

        // Ultra-fast path: if we have precomputed this scroll position and smooth scrolling is detected
        if self.high_performance_mode && self.consecutive_smooth_scrolls > 2 && self.precomputed_scrolls.contains_key(&current_scroll) {
            return true;
        }

        // Ultra-fast path: during smooth scrolling, skip content checks for small changes
        if self.consecutive_smooth_scrolls > 5 && scroll_delta <= 1 && time_since_last < 4 {
            return true; // Assume content is unchanged during rapid scrolling
        }

        // Use adaptive throttling based on detected refresh rate
        let target_fps = REFRESH_RATE_CONFIG.get_target_fps();
        let throttle_threshold = if self.high_performance_mode {
            // Aim for higher than detected refresh rate for maximum smoothness
            ((1000.0 / target_fps) * 0.8) as u64 // 80% of frame duration
        } else {
            ((1000.0 / target_fps) * 1.2) as u64 // 120% of frame duration for stability
        };

        // Valid if no content changes and small scroll changes with adaptive throttling
        self.buffer_version == get_buffer_version(buffer)
            && scroll_delta <= 2  // More sensitive for smoother scrolling
            && time_since_last < throttle_threshold
    }

    fn update(&mut self, buffer: &CosmicBuffer, scroll: usize) {
        // Measure frame time for adaptive throttling
        let frame_start = std::time::Instant::now();

        // Track scroll direction for predictive caching
        let scroll_direction = if scroll > self.cached_scroll {
            1
        } else if scroll < self.cached_scroll {
            -1
        } else {
            0
        };

        // Update smooth scrolling detection
        if scroll_direction == self.last_scroll_direction && scroll_direction != 0 {
            self.consecutive_smooth_scrolls = self.consecutive_smooth_scrolls.saturating_add(1);
        } else {
            self.consecutive_smooth_scrolls = 0;
        }
        self.last_scroll_direction = scroll_direction;

        self.cached_scroll = scroll;
        self.last_update_time = std::time::Instant::now();
        self.buffer_version = get_buffer_version(buffer);

        // Update frame time tracking
        let frame_time = frame_start.elapsed().as_millis() as u64;
        self.frame_time_samples[self.frame_time_index] = frame_time;
        self.frame_time_index = (self.frame_time_index + 1) % 8;

        // Adaptive performance adjustment
        self.adaptive_performance_update();

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

        // Predictive caching: precompute nearby scroll positions during smooth scrolling
        if self.consecutive_smooth_scrolls > 3 && self.precomputed_scrolls.len() < 50 {
            self.precompute_nearby_scrolls(scroll);
        }
    }

    fn precompute_nearby_scrolls(&mut self, current_scroll: usize) {
        // Precompute up to 20 scroll positions in the current direction for butter smooth scrolling
        let range = if self.last_scroll_direction > 0 {
            current_scroll + 1..=current_scroll + 20
        } else if self.last_scroll_direction < 0 {
            current_scroll.saturating_sub(20)..=current_scroll.saturating_sub(1)
        } else {
            return;
        };

        for scroll_pos in range {
            if !self.precomputed_scrolls.contains_key(&scroll_pos) {
                if let Some((start_line, start_offset)) = self.compute_scroll_position(scroll_pos) {
                    self.precomputed_scrolls.insert(scroll_pos, (start_line, start_offset));
                }
            }
        }
    }

    fn compute_scroll_position(&self, scroll: usize) -> Option<(usize, usize)> {
        let mut remaining = scroll;
        for (line_index, &wraps) in self.line_wraps.iter().enumerate() {
            if remaining < wraps {
                return Some((line_index, remaining));
            }
            remaining = remaining.saturating_sub(wraps);
        }
        None
    }

    fn adaptive_performance_update(&mut self) {
        // Calculate average frame time from recent samples
        let avg_frame_time: f64 = self.frame_time_samples.iter().sum::<u64>() as f64 / 8.0;

        // Dynamic target based on detected refresh rate
        let target_fps = REFRESH_RATE_CONFIG.get_target_fps();
        let target_frame_time = 1000.0 / target_fps as f64;
        let performance_ratio = avg_frame_time / target_frame_time;

        // Adaptive throttle adjustment based on detected refresh rate
        let target_fps = REFRESH_RATE_CONFIG.get_target_fps();
        let max_throttle = ((1000.0 / target_fps) * 1.5) as u64; // Allow up to 150% of frame duration

        if performance_ratio > 2.0 {
            // Poor performance relative to target refresh rate
            self.adaptive_throttle_ms = (self.adaptive_throttle_ms + 1).min(max_throttle);
            self.high_performance_mode = false;
        } else if performance_ratio < 1.2 {
            // Good performance relative to target refresh rate
            self.adaptive_throttle_ms = self.adaptive_throttle_ms.saturating_sub(1).max(1); // Min 1ms (1000Hz)
            if self.adaptive_throttle_ms <= 2 {
                self.high_performance_mode = true;
            }
        }

        // Clear precomputed cache if performance is poor - but allow larger cache for smooth scrolling
        if performance_ratio > 2.0 && self.precomputed_scrolls.len() > 50 {
            self.precomputed_scrolls.clear();
        }
    }

    fn get_visible_lines(&self, start_scroll: usize, visible_lines: usize, total_lines: usize) -> Vec<usize> {
        // Ultra-fast path: use precomputed values if available
        if let Some(&(start_line, start_offset)) = self.precomputed_scrolls.get(&start_scroll) {
            return self.compute_visible_lines_fast(start_line, start_offset, visible_lines);
        }

        // Regular path
        self.compute_visible_lines_fast(self.start_buffer_line, self.start_wrap_offset, visible_lines)
    }

    fn compute_visible_lines_fast(&self, start_buffer_line: usize, start_wrap_offset: usize, visible_lines: usize) -> Vec<usize> {
        let mut result = Vec::with_capacity(visible_lines.saturating_add(1));
        let mut current_buffer_line = start_buffer_line;
        let mut current_wrap_offset = start_wrap_offset;
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
            precomputed_text_elements: std::collections::HashMap::new(),
            text_cache_bounds: None,
            text_cache_font_size: None,
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
        // Invalidate text cache if font size changed
        if let Some(cached_font_size) = self.text_cache_font_size {
            if (cached_font_size - font_size).abs() > f32::EPSILON {
                self.precomputed_text_elements.clear();
                self.text_cache_font_size = None;
            }
        }
    }

    fn ensure_text_cache(&mut self, numbers: &[usize], font_size: f32, text_width: f32, line_height: f32) {
        // More aggressive caching - only rebuild if font size or dimensions changed significantly
        let should_rebuild = self.text_cache_font_size.is_none() ||
            (if let Some(cached_font_size) = self.text_cache_font_size {
                (cached_font_size - font_size).abs() > 0.1  // Less sensitive for better performance
            } else { true }) ||
            self.precomputed_text_elements.is_empty();

        if should_rebuild {
            self.text_cache_font_size = Some(font_size);
            self.precomputed_text_elements.clear();

            // Pre-allocate with capacity for growth to avoid frequent reallocations
            self.precomputed_text_elements.reserve(numbers.len().max(self.precomputed_text_elements.capacity()));

            // Create a single template to reuse - this is the key optimization
            let template = PrimitiveText {
                content: "", // Empty content - will use string caching during rendering
                bounds: Size::new(text_width, line_height),
                size: Pixels(font_size),
                line_height: LineHeight::Absolute(Pixels(line_height)),
                font: Default::default(), // Will be set during rendering
                horizontal_alignment: alignment::Horizontal::Right,
                vertical_alignment: alignment::Vertical::Top,
                shaping: Shaping::Basic,
            };

            // Only cache the template - actual line numbers will use string interning during render
            for &line_number in numbers.iter() {
                self.precomputed_text_elements.insert(line_number, template.clone());
            }
        }
    }

    fn get_or_create_text_batches(&mut self, bounds: Rectangle, base_padding: &Padding, gutter_width: f32, line_height: f32) -> &[(String, f32, f32)] {
        // Always regenerate if bounds changed significantly (window resize, etc.)
        let should_regenerate = !self.batch_valid || self.cached_text_batches.is_empty() || self.cached_text_batches.len() != self.numbers.len();

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
        // Only throttle if content hasn't changed and scroll is small
        let time_since_last = now.duration_since(self.last_render_time).as_millis();
        let small_scroll_change = if self.current_scroll > scroll {
            self.current_scroll - scroll <= 1  // More sensitive for smooth scrolling
        } else {
            scroll - self.current_scroll <= 1
        };

        // Always update if scroll changed significantly or content changed
        if !small_scroll_change || self.buffer_version != get_buffer_version(buffer) || (self.font_size - font_size).abs() > f32::EPSILON {
            return true;
        }

        // Optimized for detected refresh rate: allow updates every frame for ultra smooth scrolling
        let target_fps = REFRESH_RATE_CONFIG.get_target_fps();
        let frame_duration_ms = (1000.0 / target_fps) as u128;
        time_since_last >= frame_duration_ms
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
    streaming_buffer: Rc<RefCell<StreamingBuffer>>,
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
            &self.streaming_buffer,
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


// Streaming buffer for truly large files - only loads visible lines
#[derive(Debug)]
struct StreamingBuffer {
    // The complete file content (as bytes or string) - this stays on disk
    file_content: Option<String>,
    // Currently loaded lines in memory (small viewport window)
    loaded_lines: Vec<String>,
    // Starting line number in the file for loaded_lines[0]
    loaded_start_line: usize,
    // Number of lines currently loaded
    loaded_count: usize,
    // Total lines in the file
    total_lines: usize,
    // Maximum lines to keep in memory
    max_loaded_lines: usize,
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
            loaded_lines: Vec::with_capacity(200), // Start with small viewport
            loaded_start_line: 0,
            loaded_count: 0,
            total_lines: 0,
            max_loaded_lines: 500, // Only keep 500 lines in memory
            string_pool,
            render_batch: VecDeque::with_capacity(100), // Small render batch
            last_viewport: None,
        }
    }

    fn load_file_content(&mut self, content: &str) {
        self.file_content = Some(content.to_string());
        self.total_lines = content.lines().count();
        self.loaded_count = 0;
        self.loaded_start_line = 0;
        self.loaded_lines.clear();
    }

    fn ensure_lines_loaded(&mut self, target_line: usize, visible_lines: usize) {
        if self.file_content.is_none() {
            return;
        }

        // Calculate the range we need to have loaded
        let buffer_above = 50; // Load 50 lines above viewport
        let buffer_below = 50; // Load 50 lines below viewport
        let needed_start = target_line.saturating_sub(buffer_above);
        let needed_end = (target_line + visible_lines + buffer_below).min(self.total_lines);
        let needed_count = needed_end - needed_start;

        // Check if we need to load new lines
        let need_reload = self.loaded_count == 0 ||
            needed_start < self.loaded_start_line ||
            needed_end > self.loaded_start_line + self.loaded_count ||
            needed_count > self.max_loaded_lines;

        if !need_reload {
            return; // Already have the lines we need
        }

        // Clear current loaded lines
        self.loaded_lines.clear();

        // Load the needed range from file content
        if let Some(ref content) = self.file_content {
            let lines: Vec<&str> = content.lines().skip(needed_start).take(needed_count).collect();
            let line_count = lines.len();

            for line in lines {
                self.loaded_lines.push(line.to_string());
            }

            self.loaded_start_line = needed_start;
            self.loaded_count = line_count;
        }
    }

    fn get_loaded_line(&self, line_number: usize) -> Option<&str> {
        if line_number < self.loaded_start_line ||
           line_number >= self.loaded_start_line + self.loaded_count {
            return None;
        }

        let index = line_number - self.loaded_start_line;
        self.loaded_lines.get(index).map(|s| s.as_str())
    }

    fn prepare_viewport_batch(&mut self, start_line: usize, visible_lines: usize, viewport: &Rectangle, bounds: Rectangle, line_height: f32) {
        let viewport_top = viewport.y;
        let viewport_bottom = viewport.y + viewport.height;
        let start_y = bounds.y + 10.0; // Approximate base padding top

        // Clear previous batch
        self.render_batch.clear();

        // Calculate which lines are actually visible
        let start_visible_line = ((viewport_top - start_y) / line_height).floor() as usize;
        let end_visible_line = ((viewport_bottom - start_y) / line_height).ceil() as usize + 1;

        // Ensure we have the needed lines loaded
        self.ensure_lines_loaded(start_line, visible_lines);

        // Prepare batch with only visible lines that we have loaded
        for line_offset in 0..visible_lines {
            let line_num = start_line + line_offset;

            // Skip if we don't have this line loaded
            if let Some(line_content) = self.get_loaded_line(line_num) {
                let y = start_y + line_offset as f32 * line_height;
                let text_bottom = y + line_height;

                // Skip if not in viewport (extra culling)
                if text_bottom < viewport_top || y > viewport_bottom {
                    continue;
                }

                let line_str = if line_num <= 10000 {
                    self.string_pool[line_num - 1].clone()
                } else {
                    line_num.to_string()
                };

                self.render_batch.push_back((line_num, line_str));
            }
        }

        self.last_viewport = Some(*viewport);
    }

    fn render_batch(&mut self, renderer: &mut IcedRenderer, bounds: Rectangle, base_padding: &Padding, gutter_width: f32, color: Color, font_size: f32, line_height: f32, viewport: &Rectangle, start_line: usize) {
        let text_size = Pixels(font_size);
        let font = renderer.default_font();
        let text_width = (gutter_width - GUTTER_TEXT_PADDING * 2.0).max(0.0);
        let gutter_right = bounds.x + base_padding.left + gutter_width;
        let start_y = bounds.y + base_padding.top;

        // Batch render all visible lines
        while let Some((line_number, line_str)) = self.render_batch.pop_front() {
            let index = line_number.saturating_sub(start_line); // Convert to 0-based index
            let y = start_y + index as f32 * line_height;

            let text = PrimitiveText {
                content: &line_str,
                bounds: Size::new(text_width, line_height),
                size: text_size,
                line_height: LineHeight::Absolute(Pixels(line_height)),
                font,
                horizontal_alignment: alignment::Horizontal::Right,
                vertical_alignment: alignment::Vertical::Top,
                shaping: Shaping::Basic,
            };

            let x = (gutter_right - text_width - GUTTER_TEXT_PADDING).max(bounds.x);
            renderer.fill_text(text, Point::new(x, y), color, *viewport);
        }
    }

    fn get_total_lines(&self) -> usize {
        self.total_lines
    }

    fn is_large_file(&self) -> bool {
        self.total_lines > 1000 // Consider files > 1000 lines as "large"
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
    streaming_buffer: &Rc<RefCell<StreamingBuffer>>,
) {
    let _editor_ref = borrow_editor(content);
    let buffer = _editor_ref.buffer();
    let font_size = font_size_override.unwrap_or(buffer.metrics().font_size);
    let scroll = buffer.scroll().max(0) as usize;

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

    // Use streaming buffer for large files to only load visible lines
    let total_lines = buffer.lines.len();

    // For now, skip streaming buffer for files with cosmic-text integration
    // TODO: Implement proper cosmic-text buffer streaming
    if total_lines > 1000 && false { // Disabled for now
        let mut stream_buffer = streaming_buffer.borrow_mut();
        // TODO: Extract text from cosmic-text buffer properly
        stream_buffer.prepare_viewport_batch(scroll, visible_lines, viewport, bounds, line_height);
        stream_buffer.render_batch(renderer, bounds, base_padding, gutter_width, color, font_size, line_height, viewport, scroll);
    } else {
        // Small file: use traditional rendering
        let text_size = Pixels(font_size);
        let font = renderer.default_font();
        let text_width = (gutter_width - GUTTER_TEXT_PADDING * 2.0).max(0.0);

        // Calculate text positioning
        let gutter_right = bounds.x + base_padding.left + gutter_width;
        let start_y = bounds.y + base_padding.top;

        // Update text cache if needed
        line_numbers_cache.ensure_text_cache(&numbers, font_size, text_width, line_height);

        // Viewport culling: only render line numbers that are actually visible
        let viewport_top = viewport.y;
        let viewport_bottom = viewport.y + viewport.height;

        // Fast rendering for small files
        for (index, line_number) in numbers.iter().enumerate() {
            let y = start_y + index as f32 * line_height;
            let text_bottom = y + line_height;

            // Skip if text is completely outside viewport
            if text_bottom < viewport_top || y > viewport_bottom {
                continue;
            }

            // Create text with minimal allocations
            let line_str = line_number.to_string();
            let text = PrimitiveText {
                content: &line_str,
                bounds: Size::new(text_width, line_height),
                size: text_size,
                line_height: LineHeight::Absolute(Pixels(line_height)),
                font,
                horizontal_alignment: alignment::Horizontal::Right,
                vertical_alignment: alignment::Vertical::Top,
                shaping: Shaping::Basic,
            };
            let x = (gutter_right - text_width - GUTTER_TEXT_PADDING).max(bounds.x);
            renderer.fill_text(text, Point::new(x, y), color, *viewport);
        }
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
