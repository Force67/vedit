/// Viewport configuration for rendering large files
#[derive(Debug, Clone)]
pub struct Viewport {
    pub start_line: usize,
    pub visible_lines: usize,
    pub line_height: f32,
    pub buffer_capacity: usize,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            start_line: 0,
            visible_lines: 100,
            line_height: 1.5,
            buffer_capacity: 1000, // Keep ~1000 lines in memory
        }
    }
}
