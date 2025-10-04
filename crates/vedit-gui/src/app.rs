use crate::commands;
use crate::message::Message;
use crate::state::EditorState;
use crate::view;
use iced::{executor, theme, Application, Command, Element, Settings};

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
        }

        Command::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        view::view(&self.state)
    }

    fn theme(&self) -> Self::Theme {
        theme::Theme::Dark
    }
}
