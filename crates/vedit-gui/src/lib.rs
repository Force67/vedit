use iced::alignment::{Horizontal, Vertical};
use iced::widget::lazy;
use iced::widget::text_editor::{self, Action as TextEditorAction, Content};
use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{executor, theme, Alignment, Application, Command, Element, Length, Padding, Settings};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
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
    buffer_content: Content,
}

#[derive(Debug, Clone)]
enum Message {
    OpenFileRequested,
    FileLoaded(Result<Option<Document>, String>),
    DocumentSelected(usize),
    WorkspaceOpenRequested,
    WorkspaceLoaded(Result<Option<(String, Vec<FileNode>)>, String>),
    WorkspaceFileActivated(String),
    BufferAction(TextEditorAction),
}

#[derive(Clone)]
struct WorkspaceSnapshot {
    version: u64,
    tree: Arc<Vec<FileNode>>,
}

impl WorkspaceSnapshot {
    fn new(version: u64, tree: Arc<Vec<FileNode>>) -> Self {
        Self { version, tree }
    }
}

impl std::fmt::Debug for WorkspaceSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceSnapshot")
            .field("version", &self.version)
            .field("tree_entries", &self.tree.len())
            .finish()
    }
}

impl Hash for WorkspaceSnapshot {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.version.hash(state);
        (Arc::as_ptr(&self.tree) as usize).hash(state);
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
                return Command::perform(Self::pick_document(), Message::FileLoaded);
            }
            Message::FileLoaded(result) => match result {
                Ok(Some(document)) => {
                    self.editor.open_document(document);
                    self.error = None;
                    self.sync_buffer_content();
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
                self.sync_buffer_content();
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
            Message::BufferAction(action) => {
                let is_edit = action.is_edit();
                self.buffer_content.perform(action);

                if is_edit {
                    let updated = self.editor_contents_to_string();
                    self.editor.update_active_buffer(updated);
                }
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let banner = text(startup_banner())
            .size(28)
            .horizontal_alignment(Horizontal::Center);

        let buffer = text_editor::TextEditor::new(&self.buffer_content)
            .on_action(Message::BufferAction)
            .height(Length::Fill)
            .padding(12);

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

        let workspace_contents: Element<'_, Message> = if let Some((version, tree)) =
            self.editor.workspace_snapshot()
        {
            let snapshot = WorkspaceSnapshot::new(version, tree);
            lazy(snapshot, |snapshot| -> Element<'static, Message> {
                scrollable(Self::render_workspace_nodes(snapshot.tree.as_slice(), 0))
                    .height(Length::Fill)
                    .into()
            })
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
        let mut app = Self {
            editor: Editor::new(),
            error: None,
            buffer_content: Content::new(),
        };

        app.sync_buffer_content();

        app
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

    fn sync_buffer_content(&mut self) {
        let contents = self
            .editor
            .active_document()
            .map(|doc| doc.buffer.clone())
            .unwrap_or_default();

        self.buffer_content = Content::with_text(&contents);
    }

    fn editor_contents_to_string(&self) -> String {
        let mut text = self.buffer_content.text();
        if text.ends_with('\n') {
            text.pop();
        }
        text
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

    fn render_workspace_nodes(nodes: &[FileNode], indent: u16) -> Column<'static, Message> {
        nodes.iter().fold(Column::new().spacing(4), |column, node| {
            column.push(Self::render_workspace_node(node, indent))
        })
    }

    fn render_workspace_node(node: &FileNode, indent: u16) -> Element<'static, Message> {
        let label = if node.is_directory {
            format!("{}/", node.name)
        } else {
            node.name.clone()
        };

        let entry: Element<'static, Message> = if node.is_directory {
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
