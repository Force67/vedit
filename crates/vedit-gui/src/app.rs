use crate::commands;
use crate::keyboard;
use crate::message::Message;
use crate::quick_commands::QuickCommandId;
use crate::state::EditorState;
use crate::view;
use iced::Subscription;
use iced::{executor, theme, Application, Command, Element, Settings};
use iced::event;
use vedit_core::{Document, Key, QUICK_COMMAND_MENU_ACTION};

pub fn run() -> iced::Result {
    EditorApp::run(Settings::default())
}

struct EditorApp {
    state: EditorState,
}

impl Default for EditorApp {
    fn default() -> Self {
        Self {
            state: EditorState::default(),
        }
    }
}

impl Application for EditorApp {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = theme::Theme;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (Self::default(), Command::none())
    }

    fn title(&self) -> String {
        "vedit".into()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::OpenFileRequested => {
                return Command::perform(commands::pick_document(), Message::FileLoaded);
            }
            Message::FileLoaded(result) => match result {
                Ok(Some(document)) => {
                    self.state.editor_mut().open_document(document);
                    self.state.clear_error();
                    self.state.sync_buffer_from_editor();
                }
                Ok(None) => {
                    // user cancelled dialog
                }
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::DocumentSelected(index) => {
                self.state.editor_mut().set_active(index);
                self.state.sync_buffer_from_editor();
            }
            Message::WorkspaceOpenRequested => {
                return Command::perform(commands::pick_workspace(), Message::WorkspaceLoaded);
            }
            Message::WorkspaceLoaded(result) => match result {
                Ok(Some((root, tree))) => {
                    self.state.editor_mut().set_workspace(root, tree);
                    self.state.clear_error();
                }
                Ok(None) => {
                    // user cancelled dialog
                }
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::WorkspaceFileActivated(path) => {
                return Command::perform(commands::load_document_from_path(path), |result| {
                    Message::FileLoaded(result.map(Some))
                });
            }
            Message::BufferAction(action) => {
                self.state.apply_buffer_action(action);
            }
            Message::Keyboard(key_event) => {
                if let Some(core_event) = keyboard::key_event_from_iced(&key_event) {
                    if self.state.matches_action(QUICK_COMMAND_MENU_ACTION, &core_event) {
                        if self.state.command_palette().is_open() {
                            self.state.close_command_palette();
                        } else {
                            self.state.set_command_palette_query(String::new());
                            self.state.open_command_palette();
                        }
                        return Command::none();
                    }

                    if self.state.command_palette().is_open() {
                        match core_event.key {
                            Key::ArrowDown => {
                                self.state.handle_quick_command_navigation(1);
                                return Command::none();
                            }
                            Key::ArrowUp => {
                                self.state.handle_quick_command_navigation(-1);
                                return Command::none();
                            }
                            Key::Enter => {
                                if let Some(command) = self.state.selected_quick_command() {
                                    self.state.close_command_palette();
                                    return self.execute_quick_command(command);
                                }
                                return Command::none();
                            }
                            Key::Escape => {
                                self.state.close_command_palette();
                                return Command::none();
                            }
                            _ => {}
                        }
                    }
                }
            }
            Message::CommandPaletteInputChanged(query) => {
                self.state.set_command_palette_query(query);
            }
            Message::CommandPaletteCommandInvoked(command_id) => {
                self.state.close_command_palette();
                return self.execute_quick_command(command_id);
            }
            Message::CommandPaletteClosed => {
                self.state.close_command_palette();
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        view::view(&self.state)
    }

    fn theme(&self) -> Self::Theme {
        theme::Theme::Dark
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        event::listen_with(|event, _status| match event {
            event::Event::Keyboard(key_event) => Some(Message::Keyboard(key_event)),
            _ => None,
        })
    }
}

impl EditorApp {
    fn execute_quick_command(&mut self, command: QuickCommandId) -> Command<Message> {
        match command {
            QuickCommandId::OpenFile => {
                Command::perform(commands::pick_document(), Message::FileLoaded)
            }
            QuickCommandId::OpenFolder => {
                Command::perform(commands::pick_workspace(), Message::WorkspaceLoaded)
            }
            QuickCommandId::NewScratchBuffer => {
                let index = self.state.editor_mut().open_document(Document::default());
                self.state.editor_mut().set_active(index);
                self.state.clear_error();
                self.state.sync_buffer_from_editor();
                Command::none()
            }
        }
    }
}
