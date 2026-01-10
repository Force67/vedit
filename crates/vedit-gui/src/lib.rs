#[macro_use]
mod editor_log;

mod app;
mod commands;
mod console;
mod debugger;
mod keyboard;
mod message;
mod notifications;
mod scaling;
mod session;
mod state;
mod style;
mod syntax;
mod utils;
mod views;
mod widgets;

pub use app::run;
