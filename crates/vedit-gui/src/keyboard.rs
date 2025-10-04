use iced::keyboard::{key, Event, Key as IcedKey};
use vedit_core::{Key, KeyEvent};

pub fn key_event_from_iced(event: &Event) -> Option<KeyEvent> {
    match event {
        Event::KeyPressed { key, modifiers, .. } => map_key(key.clone()).map(|mapped| {
            KeyEvent::new(
                mapped,
                modifiers.control(),
                modifiers.shift(),
                modifiers.alt(),
                modifiers.logo(),
            )
        }),
        _ => None,
    }
}

fn map_key(key: IcedKey) -> Option<Key> {
    match key {
        IcedKey::Character(value) => value.chars().next().map(|ch| Key::Character(ch.to_ascii_uppercase())),
        IcedKey::Named(named) => match named {
            key::Named::ArrowDown => Some(Key::ArrowDown),
            key::Named::ArrowUp => Some(Key::ArrowUp),
            key::Named::ArrowLeft => Some(Key::ArrowLeft),
            key::Named::ArrowRight => Some(Key::ArrowRight),
            key::Named::Enter => Some(Key::Enter),
            key::Named::Escape => Some(Key::Escape),
            key::Named::Space => Some(Key::Space),
            key::Named::Tab => Some(Key::Tab),
            key::Named::Backspace => Some(Key::Backspace),
            key::Named::F1 => Some(Key::Function(1)),
            key::Named::F2 => Some(Key::Function(2)),
            key::Named::F3 => Some(Key::Function(3)),
            key::Named::F4 => Some(Key::Function(4)),
            key::Named::F5 => Some(Key::Function(5)),
            key::Named::F6 => Some(Key::Function(6)),
            key::Named::F7 => Some(Key::Function(7)),
            key::Named::F8 => Some(Key::Function(8)),
            key::Named::F9 => Some(Key::Function(9)),
            key::Named::F10 => Some(Key::Function(10)),
            key::Named::F11 => Some(Key::Function(11)),
            key::Named::F12 => Some(Key::Function(12)),
            _ => None,
        },
        IcedKey::Unidentified => None,
    }
}
