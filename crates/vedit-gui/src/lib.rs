use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{executor, theme, Alignment, Application, Command, Element, Length, Settings};
use rfd::FileDialog;
use vedit_core::{startup_banner, Document, Editor};

/// Launches the iced-powered editor application.
pub fn run() -> iced::Result {
    EditorApp::run(Settings::default())
}

#[derive(Debug)]
struct EditorApp {
    editor: Editor,
    error: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    BufferChanged(String),
    OpenFileRequested,
    FileLoaded(Result<Option<Document>, String>),
    DocumentSelected(usize),
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
            Message::BufferChanged(contents) => {
                self.editor.update_active_buffer(contents);
            }
            Message::OpenFileRequested => {
                return Command::perform(Self::pick_document(), Message::FileLoaded);
            }
            Message::FileLoaded(result) => match result {
                Ok(Some(document)) => {
                    self.editor.open_document(document);
                    self.error = None;
                }
                Ok(None) => {
                    // user cancelled dialog
                }
                Err(err) => {
                    self.error = Some(err);
                }
            },
            Message::DocumentSelected(index) => {
                self.editor.set_active(index);
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let banner = text(startup_banner())
            .size(28)
            .horizontal_alignment(Horizontal::Center);

        let active_buffer = self
            .editor
            .active_document()
            .map(|doc| doc.buffer.clone())
            .unwrap_or_default();

        let buffer = text_input("Start typing...", &active_buffer)
            .on_input(Message::BufferChanged)
            .padding(12)
            .size(16)
            .width(Length::Fill);

        let editor_panel = column![
            text("Active Buffer").size(16),
            container(buffer)
                .padding(4)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(theme::Container::Box),
        ]
        .spacing(12)
        .padding(16)
        .width(Length::FillPortion(3))
        .height(Length::Fill);

        let mut documents_column = column![text("Open Files").size(16)]
            .spacing(8)
            .padding([0, 0, 8, 0]);

        for (index, document) in self.editor.open_documents().iter().enumerate() {
            let is_active = index == self.editor.active_index();
            let mut label = document.display_name().to_string();
            if document.is_modified {
                label.push('*');
            }
            if is_active {
                label = format!("• {}", label);
            }

            let mut entry = button(text(label).size(14))
                .width(Length::Fill)
                .on_press(Message::DocumentSelected(index));

            if is_active {
                entry = entry.style(theme::Button::Primary);
            }

            documents_column = documents_column.push(entry);
        }

        let file_tree = container(scrollable(documents_column).height(Length::Fill))
            .padding(16)
            .width(Length::FillPortion(1))
            .style(theme::Container::Box);

        let content_row = row![editor_panel, file_tree]
            .spacing(16)
            .width(Length::Fill)
            .height(Length::Fill);

        let top_bar = container(
            row![
                text("vedit").size(20),
                button(text("Open File…")).on_press(Message::OpenFileRequested),
            ]
            .spacing(16)
            .align_items(Alignment::Center),
        )
        .padding([12, 16])
        .width(Length::Fill)
        .style(theme::Container::Box);

        let status_bar = container(
            row![
                text(format!("File: {}", self.editor.status_line())).size(14),
                text(format!(
                    "Chars: {}",
                    self.editor
                        .active_document()
                        .map(|doc| doc.buffer.chars().count())
                        .unwrap_or(0)
                ))
                .size(14),
                if let Some(err) = &self.error {
                    text(format!("Error: {}", err)).size(14)
                } else {
                    text("").size(14)
                },
            ]
            .spacing(24)
            .align_items(Alignment::Center),
        )
        .padding([10, 16])
        .width(Length::Fill)
        .align_y(Vertical::Center);

        container(
            column![top_bar, banner, content_row, status_bar]
                .spacing(16)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_items(Alignment::Start),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x()
        .center_y()
        .into()
    }

    fn theme(&self) -> Self::Theme {
        theme::Theme::Dark
    }
}

impl Default for EditorApp {
    fn default() -> Self {
        Self {
            editor: Editor::new(),
            error: None,
        }
    }
}

impl EditorApp {
    fn pick_document(
    ) -> impl std::future::Future<Output = Result<Option<Document>, String>> + Send + 'static {
        async move {
            let handle = FileDialog::new().pick_file();
            if let Some(path) = handle {
                let contents = std::fs::read_to_string(&path)
                    .map_err(|err| format!("Failed to read file: {}", err))?;
                let document = Document::new(
                    Some(path.to_string_lossy().to_string()),
                    contents,
                );
                Ok(Some(document))
            } else {
                Ok(None)
            }
        }
    }
}
