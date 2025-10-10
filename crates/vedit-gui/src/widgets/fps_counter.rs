use iced::widget::{container, text};
use iced::{Element, Color};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct FpsCounter {
    frame_times: Vec<Instant>,
    last_update: Instant,
    fps: f32,
}

impl Default for FpsCounter {
    fn default() -> Self {
        Self {
            frame_times: Vec::with_capacity(60),
            last_update: Instant::now(),
            fps: 0.0,
        }
    }
}

impl FpsCounter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        self.frame_times.push(now);

        // Keep only the last second of frame times
        let one_second_ago = now - Duration::from_secs(1);
        self.frame_times.retain(|&time| time > one_second_ago);

        // Calculate FPS every 100ms to avoid jittery display
        if now - self.last_update >= Duration::from_millis(100) {
            self.fps = self.frame_times.len() as f32;
            self.last_update = now;
        }
    }

    pub fn view(&self) -> Element<'_, crate::message::Message> {
        let fps_text = if self.fps >= 60.0 {
            text(format!("{:.0} FPS", self.fps))
                .style(Color::from_rgb(0.0, 1.0, 0.0)) // Green for good FPS
        } else if self.fps >= 30.0 {
            text(format!("{:.0} FPS", self.fps))
                .style(Color::from_rgb(1.0, 1.0, 0.0)) // Yellow for medium FPS
        } else {
            text(format!("{:.0} FPS", self.fps))
                .style(Color::from_rgb(1.0, 0.0, 0.0)) // Red for low FPS
        };

        container(fps_text)
            .padding(8)
            .style(iced::theme::Container::Transparent)
            .into()
    }

    pub fn fps(&self) -> f32 {
        self.fps
    }
}