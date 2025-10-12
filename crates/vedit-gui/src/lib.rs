#[macro_use]
mod editor_log;

mod app;
mod console;
mod commands;
mod debugger;
mod message;
mod notifications;
mod scaling;
mod state;
mod views;
mod keyboard;
mod widgets;
mod style;
mod syntax;
mod utils;

pub use app::run;
