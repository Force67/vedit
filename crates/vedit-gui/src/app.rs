use crate::commands::{self, SaveDocumentRequest, SaveKeymapRequest, WorkspaceData};
use crate::keyboard;
use crate::message::Message;
use crate::quick_commands::QuickCommandId;
use crate::state::EditorState;
use crate::view;
use iced::Subscription;
use iced::{executor, theme, Application, Command, Element, Settings};
use iced::{event, mouse};
use vedit_core::{Document, Key, QUICK_COMMAND_MENU_ACTION, SAVE_ACTION};

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
                    if let Some((root, config)) = self.state.record_recent_workspace_file() {
                        return Command::perform(
                            commands::save_workspace_config(root, config),
                            Message::WorkspaceConfigSaved,
                        );
                    }
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
            Message::SolutionOpenRequested => {
                return Command::perform(commands::pick_solution(), Message::SolutionLoaded);
            }
            Message::WorkspaceLoaded(result) => match result {
                Ok(Some(WorkspaceData { root, tree, config })) => {
                    self.state.install_workspace(root.clone(), tree, config);
                    self.state.clear_error();
                }
                Ok(None) => {
                    // user cancelled dialog
                }
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::SolutionLoaded(result) => match result {
                Ok(Some(WorkspaceData { root, tree, config })) => {
                    self.state.install_workspace(root.clone(), tree, config);
                    self.state.clear_error();
                }
                Ok(None) => {}
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::WorkspaceFileActivated(path) => {
                return Command::perform(commands::load_document_from_path(path), |result| {
                    Message::FileLoaded(result.map(Some))
                });
            }
            Message::WorkspaceDirectoryToggled(path) => {
                if let Err(err) = self.state.toggle_workspace_directory(path) {
                    self.state.set_error(Some(err));
                }
            }
            Message::BufferAction(action) => {
                self.state.apply_buffer_action(action);
            }
            Message::DocumentSaved(result) => match result {
                Ok(Some(path)) => {
                    self.state.handle_document_saved(Some(path));
                    self.state.clear_error();
                }
                Ok(None) => {}
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::WorkspaceConfigSaved(result) => match result {
                Ok(root) => {
                    self.state.apply_workspace_config_saved(root);
                }
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::SettingsOpened => {
                self.state.open_settings();
            }
            Message::SettingsClosed => {
                self.state.close_settings();
            }
            Message::SettingsCategorySelected(category) => {
                self.state.settings_mut().select_category(category);
            }
            Message::SettingsBindingChanged(id, value) => {
                self.state.settings_mut().set_binding_input(id, value);
                self.state.clear_binding_error(id);
            }
            Message::SettingsBindingApplied(id) => {
                if let Err(err) = self.state.apply_quick_command_binding(id) {
                    self.state.set_error(Some(err));
                } else {
                    self.state.clear_error();
                }
            }
            Message::SettingsBindingsSaveRequested => {
                match self.state.keymap_save_payload() {
                    Ok((path, contents)) => {
                        let request = SaveKeymapRequest { path, contents };
                        return Command::perform(
                            commands::save_keymap(request),
                            Message::SettingsBindingsSaved,
                        );
                    }
                    Err(err) => {
                        self.state.set_error(Some(err));
                    }
                }
            }
            Message::SettingsBindingsSaved(result) => match result {
                Ok(path) => {
                    self.state.mark_keymap_saved(path);
                }
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::SettingsKeymapPathRequested => {
                let current = self.state.keymap_path_display();
                return Command::perform(
                    commands::pick_keymap_location(current),
                    Message::SettingsKeymapPathSelected,
                );
            }
            Message::SettingsKeymapPathSelected(result) => match result {
                Ok(Some(path)) => {
                    if let Err(err) = self.state.apply_selected_keymap_path(path) {
                        self.state.set_error(Some(err));
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    self.state.set_error(Some(err));
                }
            },
            Message::Keyboard(key_event) => {
                match key_event {
                    iced::keyboard::Event::ModifiersChanged(modifiers) => {
                        self.state.set_modifiers(modifiers);
                        return Command::none();
                    }
                    iced::keyboard::Event::KeyPressed { modifiers, .. }
                    | iced::keyboard::Event::KeyReleased { modifiers, .. } => {
                        self.state.set_modifiers(modifiers);
                    }
                }

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

                    for command in self.state.quick_commands() {
                        if let Some(action) = command.action {
                            if self.state.matches_action(action, &core_event) {
                                return self.execute_quick_command(command.id);
                            }
                        }
                    }

                    if self.state.matches_action(SAVE_ACTION, &core_event) {
                        return self.save_active_document();
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
            Message::MouseWheelScrolled(delta) => {
                let modifiers = self.state.modifiers();
                if !(modifiers.control() || modifiers.command()) {
                    return Command::none();
                }

                let delta_y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => y,
                    mouse::ScrollDelta::Pixels { y, .. } => y,
                };

                if delta_y > 0.0 {
                    self.state.increase_scale_factor();
                } else if delta_y < 0.0 {
                    self.state.decrease_scale_factor();
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
            Message::CommandPromptToggled => {
                if self.state.command_palette().is_open() {
                    self.state.close_command_palette();
                } else {
                    self.state.set_command_palette_query(String::new());
                    self.state.open_command_palette();
                }
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
            event::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                Some(Message::MouseWheelScrolled(delta))
            }
            _ => None,
        })
    }

    fn scale_factor(&self) -> f64 {
        self.state.scale_factor()
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
            QuickCommandId::OpenSolution => {
                Command::perform(commands::pick_solution(), Message::SolutionLoaded)
            }
            QuickCommandId::SaveFile => self.save_active_document(),
            QuickCommandId::NewScratchBuffer => {
                let index = self.state.editor_mut().open_document(Document::default());
                self.state.editor_mut().set_active(index);
                self.state.clear_error();
                self.state.sync_buffer_from_editor();
                Command::none()
            }
            QuickCommandId::ShowScaleFactor => {
                let scale_info = self.state.format_scale_factor();
                println!("{}", scale_info);
                self.state.set_error(Some(scale_info));
                Command::none()
            }
        }
    }

    fn save_active_document(&mut self) -> Command<Message> {
        if let Some(doc) = self.state.editor().active_document() {
            let request = SaveDocumentRequest {
                path: doc.path.clone(),
                contents: doc.buffer.clone(),
                suggested_name: Some(doc.display_name().to_string()),
            };
            Command::perform(commands::save_document(request), Message::DocumentSaved)
        } else {
            Command::none()
        }
    }
}
