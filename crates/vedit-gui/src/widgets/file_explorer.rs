use iced::widget::{button, column, container, horizontal_space, row, scrollable, text, text_input, Column, Row, Scrollable};
use iced::{Command, Element, Length, Padding, Alignment};
use iced_font_awesome::fa_icon_solid;
use crate::style;
use vedit_core::{FilterState, FsWorkspaceProvider, GitStatus, Node, NodeId, NodeKind, WorkspaceProvider, WorkspaceTree};



#[derive(Debug, Clone)]
pub enum Message {
    TreeToggle(NodeId),
    TreeSelect(NodeId, SelectKind),
    Open(NodeId, OpenKind),
    OpenFile(String),
    InlineAction(NodeId, InlineAction),
    StartRename(NodeId),
    CommitRename(NodeId, String),
    CancelRename,
    NewFile(NodeId),
    NewFolder(NodeId),
    Delete(NodeId),
    ConfirmDelete(bool),
    DragStart(NodeId),
    DragOver(NodeId),
    Drop(NodeId, DropOp),
    FilterChanged(String),
    FilterOptionToggled(FilterOpt),
    RevealActive,
    CollapseAll,
    Refresh,
    FsEvent(FsNotice),
    GitDecorations(GitMap),
    FocusNext,
    FocusPrev,
    RowClick(NodeId),
    TooltipShown(String),
    TooltipHide,
}

#[derive(Debug, Clone)]
pub enum SelectKind {
    Single,
    Range,
    Toggle,
}

#[derive(Debug, Clone)]
pub enum OpenKind {
    InEditor,
    Split,
}

#[derive(Debug, Clone)]
pub enum InlineAction {
    Rename,
    Delete,
    RevealInOs,
    CopyPath,
    OpenInTerminal,
}

#[derive(Debug, Clone)]
pub enum DropOp {
    Move,
    Copy,
}

#[derive(Debug, Clone)]
pub enum FilterOpt {
    MatchCase,
    FilesOnly,
    FoldersOnly,
    ShowHidden,
}

#[derive(Debug, Clone)]
pub struct FsNotice {
    // Placeholder for FS event
}

#[derive(Debug, Clone)]
pub struct GitMap {
    // Placeholder for git status map
}

pub struct FileExplorer {
    tree: WorkspaceTree,
    provider: FsWorkspaceProvider,
    root_path: std::path::PathBuf,
    vrows: Vec<NodeId>,
    row_height: u16,
    scroll_offset: f32,
    renaming: Option<NodeId>,
    filter_input: String,
    last_click: Option<(NodeId, std::time::Instant)>,
}

impl std::fmt::Debug for FileExplorer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileExplorer")
            .field("vrows", &self.vrows)
            .field("row_height", &self.row_height)
            .field("scroll_offset", &self.scroll_offset)
            .field("renaming", &self.renaming)
            .field("filter_input", &self.filter_input)
            .field("last_click", &self.last_click)
            .finish()
    }
}

impl FileExplorer {
    pub fn new(root_path: std::path::PathBuf) -> Self {
        let provider = FsWorkspaceProvider::new(root_path.clone());
        let mut tree = WorkspaceTree {
            root: 0,
            nodes: Default::default(),
            expanded: Default::default(),
            selection: Default::default(),
            cursor: None,
            filter: FilterState {
                query: String::new(),
                match_case: false,
                files_only: false,
                folders_only: false,
                show_hidden: false,
            },
        };

        // Build initial tree
        Self::build_initial_tree(&mut tree, &provider, &root_path);

        let vrows = Self::compute_visible_rows(&tree);

        Self {
            tree,
            provider,
            root_path: root_path.clone(),
            vrows,
            row_height: 28,
            scroll_offset: 0.0,
            renaming: None,
            filter_input: String::new(),
            last_click: None,
        }
    }

    fn build_initial_tree(tree: &mut WorkspaceTree, provider: &FsWorkspaceProvider, root_path: &std::path::Path) {
        // Add the root node
        let root_id = tree.nodes.insert(Node {
            id: 0,
            name: root_path.file_name().and_then(|n| n.to_str()).unwrap_or("root").to_string(),
            rel_path: ".".to_string(),
            kind: NodeKind::Folder,
            size: None,
            modified: None,
            children: None,
            git: None,
            is_hidden: false,
        });
        tree.root = root_id;
        // Load children for root
        provider.load_children(tree, root_id).unwrap_or(());
        // Expand root
        tree.expanded.insert(root_id);
    }

    fn compute_visible_rows(tree: &WorkspaceTree) -> Vec<NodeId> {
        let mut visible = Vec::new();
        let mut stack = vec![tree.root];
        while let Some(id) = stack.pop() {
            visible.push(id);
            if let Some(node) = tree.nodes.get(id) {
                if tree.expanded.contains(&id) {
                    if let Some(children) = &node.children {
                        for &child in children.iter().rev() {
                            stack.push(child);
                        }
                    }
                }
            }
        }
        visible
    }

    pub fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::TreeToggle(id) => {
                if self.tree.expanded.contains(&id) {
                    self.tree.expanded.remove(&id);
                } else {
                    self.provider.load_children(&mut self.tree, id).unwrap_or(());
                    self.tree.expanded.insert(id);
                }
                self.update_visible_rows();
                Command::none()
            }
            Message::TreeSelect(id, kind) => {
                match kind {
                    SelectKind::Single => {
                        self.tree.selection.clear();
                        self.tree.selection.insert(id);
                        self.tree.cursor = Some(id);
                        Command::none()
                    }
                    SelectKind::Range => {
                        if let Some(cursor) = self.tree.cursor {
                            self.tree.selection.clear();
                            if let Some(cursor_idx) = self.vrows.iter().position(|&i| i == cursor) {
                                if let Some(id_idx) = self.vrows.iter().position(|&i| i == id) {
                                    let start = cursor_idx.min(id_idx);
                                    let end = cursor_idx.max(id_idx);
                                    for &i in &self.vrows[start..=end] {
                                        self.tree.selection.insert(i);
                                    }
                                    self.tree.cursor = Some(id);
                                }
                            }
                        }
                        Command::none()
                    }
                    SelectKind::Toggle => {
                        if self.tree.selection.contains(&id) {
                            self.tree.selection.remove(&id);
                        } else {
                            self.tree.selection.insert(id);
                        }
                        self.tree.cursor = Some(id);
                        Command::none()
                    }
                }
            }
            Message::Open(id, kind) => {
                if let Some(node) = self.tree.nodes.get(id) {
                    if matches!(node.kind, NodeKind::Folder) {
                        if self.tree.expanded.contains(&id) {
                            self.tree.expanded.remove(&id);
                        } else {
                            self.tree.expanded.insert(id);
                        }
                        self.update_visible_rows();
                    }
                }
                Command::none()
            }
            Message::RowClick(id) => {
                if let Some(node) = self.tree.nodes.get(id) {
                    if matches!(node.kind, NodeKind::File) {
                        let full_path = self.root_path.join(&node.rel_path).to_string_lossy().to_string();
                        return Command::perform(async { Message::OpenFile(full_path) }, |msg| msg);
                    }
                }
                // For folders or other, handle selection and double click
                let now = std::time::Instant::now();
                if let Some((last_id, last_time)) = self.last_click {
                    if last_id == id && now.duration_since(last_time) < std::time::Duration::from_millis(500) {
                        // Double click
                        if let Some(node) = self.tree.nodes.get(id) {
                            if matches!(node.kind, NodeKind::Folder) {
                                if self.tree.expanded.contains(&id) {
                                    self.tree.expanded.remove(&id);
                                } else {
                                    self.provider.load_children(&mut self.tree, id).unwrap_or(());
                                    self.tree.expanded.insert(id);
                                }
                                self.update_visible_rows();
                            }
                        }
                        self.last_click = None;
                    } else {
                        self.tree.selection.clear();
                        self.tree.selection.insert(id);
                        self.tree.cursor = Some(id);
                        self.last_click = Some((id, now));
                    }
                } else {
                    self.tree.selection.clear();
                    self.tree.selection.insert(id);
                    self.tree.cursor = Some(id);
                    self.last_click = Some((id, now));
                }
                Command::none()
            }
            Message::StartRename(id) => {
                self.renaming = Some(id);
                Command::none()
            }
            Message::CommitRename(id, new_name) => {
                if self.renaming == Some(id) && !new_name.is_empty() {
                    if let Some(node) = self.tree.nodes.get_mut(id) {
                        node.name = new_name;
                    }
                }
                self.renaming = None;
                Command::none()
            }
            Message::CancelRename => {
                self.renaming = None;
                Command::none()
            }
            Message::Delete(id) => {
                self.tree.nodes.remove(id);
                self.update_visible_rows();
                Command::none()
            }
            _ => Command::none()
        }
    }

    fn update_visible_rows(&mut self) {
        self.vrows = Self::compute_visible_rows(&self.tree);
    }

    pub fn cursor(&self) -> Option<NodeId> {
        self.tree.cursor
    }

    pub fn view(&self) -> Element<Message> {
        let header = self.header();
        let tree_view = self.tree_view();

        let content = Column::new()
            .push(header)
            .push(tree_view);

        content.into()
    }

    fn header(&self) -> Element<Message> {
        let filter_input = text_input("Filter files...", &self.filter_input)
            .on_input(Message::FilterChanged);

        let actions = Row::new()
            .push(button(text("New")).style(style::custom_button()))
            .push(button(text("Refresh")).style(style::custom_button()))
            .push(button(text("Collapse All")).style(style::custom_button()))
            .push(button(text("Reveal Active")).style(style::custom_button()))
            .spacing(4);

        Column::new()
            .push(text("Workspace: /path/to/root").style(iced::theme::Text::Color(style::TEXT)))
            .push(actions)
            .push(filter_input)
            .spacing(8)
            .padding(Padding::from([8, 16]))
            .into()
    }

    fn tree_view(&self) -> Element<Message> {
        let rows: Vec<Element<Message>> = self.vrows.iter().map(|&id| self.row_view(id)).collect();

        Scrollable::new(Column::new().extend(rows))
            .style(style::custom_scrollable())
            .into()
    }

    fn get_node_depth(&self, id: NodeId) -> usize {
        let mut depth = 0;
        let mut current_id = id;
        while let Some(parent_id) = self.find_parent(current_id) {
            depth += 1;
            current_id = parent_id;
        }
        depth
    }

    fn find_parent(&self, child_id: NodeId) -> Option<NodeId> {
        for (id, node) in &self.tree.nodes {
            if let Some(children) = &node.children {
                if children.contains(&child_id) {
                    return Some(id);
                }
            }
        }
        None
    }

    fn row_view(&self, id: NodeId) -> Element<Message> {
        if let Some(node) = self.tree.nodes.get(id) {
            let is_selected = self.tree.selection.contains(&id);
            let depth = self.get_node_depth(id);

            let icon = match node.kind {
                NodeKind::Folder => fa_icon_solid("folder").color(iced::Color::WHITE).size(14.0),
                NodeKind::File => fa_icon_solid("file").color(iced::Color::WHITE).size(14.0),
                NodeKind::Symlink(_) => fa_icon_solid("link").color(iced::Color::WHITE).size(14.0),
            };

            let chevron = if matches!(node.kind, NodeKind::Folder) {
                if self.tree.expanded.contains(&id) {
                    fa_icon_solid("chevron-down").color(iced::Color::WHITE).size(12.0)
                } else {
                    fa_icon_solid("chevron-right").color(iced::Color::WHITE).size(12.0)
                }
            } else {
                fa_icon_solid("chevron-right").color(iced::Color::TRANSPARENT).size(12.0)
            };

            let name_button = button(text(&node.name).style(if is_selected { iced::theme::Text::Color(style::PRIMARY) } else { iced::theme::Text::Color(style::TEXT) }))
                .style(style::custom_button())
                .on_press(Message::RowClick(id));

            // Create indentation based on depth
            let indent_width = depth * 16; // 16 pixels per level
            let indent = if indent_width > 0 {
                horizontal_space().width(Length::Fixed(indent_width as f32))
            } else {
                horizontal_space().width(Length::Fixed(0.0))
            };

            let chevron_element: Element<Message> = if matches!(node.kind, NodeKind::Folder) {
                button(chevron.size(12.0))
                    .style(style::custom_button())
                    .on_press(Message::TreeToggle(id))
                    .into()
            } else {
                horizontal_space().width(Length::Fixed(12.0)).into()
            };

            let row = row![
                indent,
                chevron_element,
                icon.size(14.0),
                name_button,
            ]
            .spacing(4)
            .align_items(Alignment::Center);

            row.into()
        } else {
            text("Invalid node").into()
        }
    }
}
