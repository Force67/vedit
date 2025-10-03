use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Column};
use iced::{executor, theme, Alignment, Application, Command, Element, Length, Padding, Settings};
use rfd::FileDialog;
use vedit_core::{startup_banner, Document, Editor, FileNode};

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
    WorkspaceOpenRequested,
    WorkspaceLoaded(Result<Option<(String, Vec<FileNode>)>, String>),
    WorkspaceFileActivated(String),
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
            Message::WorkspaceOpenRequested => {
                return Command::perform(Self::pick_workspace(), Message::WorkspaceLoaded);
            }
            Message::WorkspaceLoaded(result) => match result {
                Ok(Some((root, tree))) => {
                    self.editor.set_workspace(root, tree);
                    self.error = None;
                }
                Ok(None) => {
                    // user cancelled dialog
                }
                Err(err) => {
                    self.error = Some(err);
                }
            },
            Message::WorkspaceFileActivated(path) => {
                return Command::perform(Self::load_document_from_path(path), |result| {
                    Message::FileLoaded(result.map(Some))
                });
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

        let open_files_section = container(documents_column)
            .padding(16)
            .width(Length::Fill)
            .style(theme::Container::Box);

        let workspace_title = if let Some(root) = self.editor.workspace_root() {
            format!("Workspace: {}", root)
        } else {
            "Workspace".to_string()
        };

        let workspace_contents: Element<'_, Message> = if let Some(tree) = self.editor.workspace_tree() {
            scrollable(Self::render_workspace_nodes(tree, 0))
                .height(Length::Fill)
                .into()
        } else {
            column![text("Open a folder to browse project files").size(14)]
                .width(Length::Fill)
                .height(Length::Shrink)
                .into()
        };

        let workspace_section = container(
            column![text(workspace_title).size(16), workspace_contents]
                .spacing(8)
                .height(Length::Fill),
        )
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::Container::Box);

        let side_panel = column![open_files_section, workspace_section]
            .spacing(16)
            .width(Length::FillPortion(2))
            .height(Length::Fill);

        let content_row = row![editor_panel, side_panel]
            .spacing(16)
            .width(Length::Fill)
            .height(Length::Fill);

        let top_bar = container(
            row![
                text("vedit").size(20),
                button(text("Open File…")).on_press(Message::OpenFileRequested),
                button(text("Open Folder…")).on_press(Message::WorkspaceOpenRequested),
            ]
            .spacing(16)
            .align_items(Alignment::Center),
        )
        .padding([12, 16])
        .width(Length::Fill)
        .style(theme::Container::Box);

        let workspace_status = format!(
            "Workspace: {}",
            self.editor
                .workspace_root()
                .unwrap_or("(none)")
        );

        let status_bar = container(
            row![
                text(format!("File: {}", self.editor.status_line())).size(14),
                text(workspace_status).size(14),
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
            if let Some(path) = FileDialog::new().pick_file() {
                let document = Document::from_path(&path)
                    .map_err(|err| format!("Failed to read file: {}", err))?;
                Ok(Some(document))
            } else {
                Ok(None)
            }
        }
    }

    fn load_document_from_path(
        path: String,
    ) -> impl std::future::Future<Output = Result<Document, String>> + Send + 'static {
        async move { Document::from_path(&path).map_err(|err| format!("Failed to read file: {}", err)) }
    }

    fn pick_workspace(
    ) -> impl std::future::Future<Output = Result<Option<(String, Vec<FileNode>)>, String>> + Send + 'static {
        async move {
            if let Some(path) = FileDialog::new().pick_folder() {
                let tree = Editor::build_workspace_tree(&path)
                    .map_err(|err| format!("Failed to read folder: {}", err))?;
                Ok(Some((path.to_string_lossy().to_string(), tree)))
            } else {
                Ok(None)
            }
        }
    }

    fn render_workspace_nodes<'a>(
        nodes: &'a [FileNode],
        indent: u16,
    ) -> Column<'a, Message> {
        nodes.iter().fold(Column::new().spacing(4), |column, node| {
            column.push(Self::render_workspace_node(node, indent))
        })
    }

    fn render_workspace_node<'a>(node: &'a FileNode, indent: u16) -> Element<'a, Message> {
        let label = if node.is_directory {
            format!("{}/", node.name)
        } else {
            node.name.clone()
        };

        let entry: Element<'_, Message> = if node.is_directory {
            text(label).size(14).into()
        } else {
            button(text(label).size(14))
                .style(theme::Button::Text)
                .width(Length::Fill)
                .on_press(Message::WorkspaceFileActivated(node.path.clone()))
                .into()
        };

        let indent_padding = indent.saturating_mul(16);

        let mut column = Column::new();
        column = column.push(
            container(entry).padding(Padding::from([0, 0, 0, indent_padding])),
        );

        if node.is_directory && !node.children.is_empty() {
            column = column.push(Self::render_workspace_nodes(&node.children, indent + 1));
        }

        column.into()
    }
}
