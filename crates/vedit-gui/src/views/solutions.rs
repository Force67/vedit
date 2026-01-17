//! Visual Studio-style Solution Explorer view.
//!
//! This module renders solutions and projects exactly like Visual Studio's
//! Solution Explorer, with collapsible tree nodes, virtual folders for
//! source/header/resource files, and proper icons.

use crate::message::Message;
use crate::state::{
    EditorState, MakefileEntry, ProjectReferenceEntry, SolutionBrowserEntry, SolutionErrorEntry,
    SolutionTreeNode, VisualStudioProjectEntry, VisualStudioSolutionEntry,
};
use crate::style::{self, CHEVRON_COLOR, ERROR, FILE_ICON, FOLDER_ICON, MUTED, TEXT, WARNING};
use iced::widget::{Column, Row, Space, button, column, row, scrollable, text};
use iced::{Alignment, Element, Length, Padding};
use iced_font_awesome::fa_icon_solid;

// VS-style colors
const PROJECT_APP_COLOR: iced::Color = iced::Color {
    r: 0.4,
    g: 0.7,
    b: 0.4,
    a: 1.0,
}; // Green for executables
const PROJECT_LIB_COLOR: iced::Color = iced::Color {
    r: 0.6,
    g: 0.5,
    b: 0.8,
    a: 1.0,
}; // Purple for libraries
const REFERENCE_COLOR: iced::Color = iced::Color {
    r: 0.5,
    g: 0.6,
    b: 0.8,
    a: 1.0,
}; // Blue for references

// Indentation per tree level (in pixels)
const INDENT_PX: f32 = 16.0;
// Row height
const ROW_HEIGHT: f32 = 22.0;

pub fn render_solutions_tab(state: &EditorState, scale: f32) -> Column<'_, Message> {
    let entries = state.workspace_solutions();

    if entries.is_empty() {
        return column![
            text("Solution Explorer")
                .size((14.0 * scale).max(11.0))
                .color(TEXT),
            Space::new().height(8.0),
            text("No solutions found")
                .color(MUTED)
                .size((12.0 * scale).max(9.0)),
            text("Open a .sln file or workspace")
                .color(MUTED)
                .size((11.0 * scale).max(8.0)),
        ]
        .spacing(4.0)
        .padding(Padding::from([8.0, 12.0]));
    }

    let mut content = Column::new()
        .spacing(0.0)
        .padding(Padding::from([4.0, 0.0]));

    for entry in entries {
        content = content.push(render_entry(state, entry, scale));
    }

    column![scrollable(content).height(Length::Fill)]
}

fn render_entry<'a>(
    state: &'a EditorState,
    entry: &'a SolutionBrowserEntry,
    scale: f32,
) -> Element<'a, Message> {
    match entry {
        SolutionBrowserEntry::VisualStudio(solution) => render_vs_solution(state, solution, scale),
        SolutionBrowserEntry::Makefile(makefile) => render_makefile(state, makefile, scale),
        SolutionBrowserEntry::Error(error) => render_error(error, scale),
    }
}

// ============================================================================
// Visual Studio Solution Rendering
// ============================================================================

fn render_vs_solution<'a>(
    state: &'a EditorState,
    solution: &'a VisualStudioSolutionEntry,
    scale: f32,
) -> Element<'a, Message> {
    let node_id = format!("sln:{}", solution.path);
    let is_expanded = state.is_solution_node_expanded(&node_id);

    let mut rows: Vec<Element<'a, Message>> = Vec::new();

    // Solution header row
    let project_count = solution.projects.len();
    let label = format!(
        "Solution '{}' ({} project{})",
        solution.name,
        project_count,
        if project_count == 1 { "" } else { "s" }
    );

    rows.push(tree_row(
        0,
        is_expanded,
        true, // has children
        fa_icon_solid("briefcase")
            .size((12.0 * scale).max(9.0))
            .color(FOLDER_ICON),
        text(label).size((13.0 * scale).max(10.0)).color(TEXT),
        Some(Message::SolutionTreeToggle(node_id.clone())),
        Some(Message::SolutionSelected(solution.path.clone())),
        scale,
    ));

    // Solution children (only if expanded)
    if is_expanded {
        // Show warnings first
        for warning in &solution.warnings {
            rows.push(
                row![
                    Space::new().width(INDENT_PX * 2.0),
                    fa_icon_solid("triangle-exclamation")
                        .size((10.0 * scale).max(8.0))
                        .color(WARNING),
                    text(warning).size((11.0 * scale).max(8.0)).color(WARNING),
                ]
                .spacing(4.0)
                .align_y(Alignment::Center)
                .height(ROW_HEIGHT * scale)
                .into(),
            );
        }

        // Render each project
        for project in &solution.projects {
            rows.extend(render_vs_project(state, project, 1, scale));
        }
    }

    Column::with_children(rows).spacing(0.0).into()
}

fn render_vs_project<'a>(
    state: &'a EditorState,
    project: &'a VisualStudioProjectEntry,
    depth: usize,
    scale: f32,
) -> Vec<Element<'a, Message>> {
    let node_id = format!("proj:{}", project.path);
    let is_expanded = state.is_solution_node_expanded(&node_id);

    let mut rows: Vec<Element<'a, Message>> = Vec::new();

    // Determine project icon and color based on type
    let (icon_name, icon_color) = match project.project_type.as_deref() {
        Some("Application") => ("crosshairs", PROJECT_APP_COLOR),
        Some("Dynamic Library") => ("cube", PROJECT_LIB_COLOR),
        Some("Static Library") => ("cubes", PROJECT_LIB_COLOR),
        Some("Utility") => ("wrench", MUTED),
        _ => ("file-code", FILE_ICON),
    };

    // Build project label with optional toolset
    let label = if let Some(ref toolset) = project.platform_toolset {
        format!("{} [{}]", project.name, toolset)
    } else {
        project.name.clone()
    };

    let has_content =
        !project.files.is_empty() || !project.references.is_empty() || project.load_error.is_some();

    rows.push(tree_row(
        depth,
        is_expanded,
        has_content,
        fa_icon_solid(icon_name)
            .size((12.0 * scale).max(9.0))
            .color(icon_color),
        text(label).size((13.0 * scale).max(10.0)).color(TEXT),
        Some(Message::SolutionTreeToggle(node_id.clone())),
        Some(Message::WorkspaceFileActivated(project.path.clone())),
        scale,
    ));

    if is_expanded {
        // Show error if any
        if let Some(error) = &project.load_error {
            rows.push(
                row![
                    Space::new().width(INDENT_PX * (depth + 1) as f32),
                    fa_icon_solid("circle-exclamation")
                        .size((10.0 * scale).max(8.0))
                        .color(ERROR),
                    text(error).size((11.0 * scale).max(8.0)).color(ERROR),
                ]
                .spacing(4.0)
                .align_y(Alignment::Center)
                .height(ROW_HEIGHT * scale)
                .into(),
            );
        } else {
            // Render virtual folders for categorized files
            rows.extend(render_categorized_files(
                state,
                &node_id,
                &project.files,
                depth + 1,
                scale,
            ));

            // References folder
            if !project.references.is_empty() {
                rows.extend(render_references_folder(
                    state,
                    &format!("{}:refs", node_id),
                    &project.references,
                    depth + 1,
                    scale,
                ));
            }
        }
    }

    rows
}

/// Render files organized into VS-style virtual folders (Source Files, Header Files, etc.)
fn render_categorized_files<'a>(
    state: &'a EditorState,
    project_node_id: &str,
    files: &'a [SolutionTreeNode],
    depth: usize,
    scale: f32,
) -> Vec<Element<'a, Message>> {
    let mut rows: Vec<Element<'a, Message>> = Vec::new();

    // Collect files into categories
    let mut sources: Vec<&SolutionTreeNode> = Vec::new();
    let mut headers: Vec<&SolutionTreeNode> = Vec::new();
    let mut resources: Vec<&SolutionTreeNode> = Vec::new();
    let mut others: Vec<&SolutionTreeNode> = Vec::new();

    collect_files_recursive(
        files,
        &mut sources,
        &mut headers,
        &mut resources,
        &mut others,
    );

    // Source Files folder
    if !sources.is_empty() {
        let folder_id = format!("{}:src", project_node_id);
        let is_expanded = state.is_solution_node_expanded(&folder_id);

        rows.push(tree_row(
            depth,
            is_expanded,
            true,
            fa_icon_solid("folder")
                .size((12.0 * scale).max(9.0))
                .color(FOLDER_ICON),
            text("Source Files")
                .size((12.0 * scale).max(9.0))
                .color(TEXT),
            Some(Message::SolutionTreeToggle(folder_id)),
            None,
            scale,
        ));

        if is_expanded {
            for file in sources {
                rows.extend(render_file_node(state, file, depth + 1, scale));
            }
        }
    }

    // Header Files folder
    if !headers.is_empty() {
        let folder_id = format!("{}:hdr", project_node_id);
        let is_expanded = state.is_solution_node_expanded(&folder_id);

        rows.push(tree_row(
            depth,
            is_expanded,
            true,
            fa_icon_solid("folder")
                .size((12.0 * scale).max(9.0))
                .color(FOLDER_ICON),
            text("Header Files")
                .size((12.0 * scale).max(9.0))
                .color(TEXT),
            Some(Message::SolutionTreeToggle(folder_id)),
            None,
            scale,
        ));

        if is_expanded {
            for file in headers {
                rows.extend(render_file_node(state, file, depth + 1, scale));
            }
        }
    }

    // Resource Files folder
    if !resources.is_empty() {
        let folder_id = format!("{}:res", project_node_id);
        let is_expanded = state.is_solution_node_expanded(&folder_id);

        rows.push(tree_row(
            depth,
            is_expanded,
            true,
            fa_icon_solid("folder")
                .size((12.0 * scale).max(9.0))
                .color(FOLDER_ICON),
            text("Resource Files")
                .size((12.0 * scale).max(9.0))
                .color(TEXT),
            Some(Message::SolutionTreeToggle(folder_id)),
            None,
            scale,
        ));

        if is_expanded {
            for file in resources {
                rows.extend(render_file_node(state, file, depth + 1, scale));
            }
        }
    }

    // Other files (at project root, not in a folder)
    for file in others {
        rows.extend(render_file_node(state, file, depth, scale));
    }

    rows
}

/// Recursively collect files into categories
fn collect_files_recursive<'a>(
    nodes: &'a [SolutionTreeNode],
    sources: &mut Vec<&'a SolutionTreeNode>,
    headers: &mut Vec<&'a SolutionTreeNode>,
    resources: &mut Vec<&'a SolutionTreeNode>,
    others: &mut Vec<&'a SolutionTreeNode>,
) {
    for node in nodes {
        if node.is_directory {
            // Recurse into directories
            collect_files_recursive(&node.children, sources, headers, resources, others);
        } else {
            let ext = node
                .name
                .rsplit('.')
                .next()
                .map(|e| e.to_lowercase())
                .unwrap_or_default();

            match ext.as_str() {
                // Source files
                "c" | "cpp" | "cxx" | "cc" | "c++" | "m" | "mm" | "asm" | "s" => {
                    sources.push(node);
                }
                // Header files
                "h" | "hpp" | "hxx" | "hh" | "h++" | "inc" | "inl" => {
                    headers.push(node);
                }
                // Resource files
                "rc" | "rc2" | "ico" | "cur" | "bmp" | "png" | "jpg" | "jpeg" | "gif"
                | "manifest" | "resx" => {
                    resources.push(node);
                }
                // Everything else
                _ => {
                    others.push(node);
                }
            }
        }
    }
}

fn render_references_folder<'a>(
    state: &'a EditorState,
    node_id: &str,
    references: &'a [ProjectReferenceEntry],
    depth: usize,
    scale: f32,
) -> Vec<Element<'a, Message>> {
    let is_expanded = state.is_solution_node_expanded(node_id);
    let mut rows: Vec<Element<'a, Message>> = Vec::new();

    rows.push(tree_row(
        depth,
        is_expanded,
        !references.is_empty(),
        fa_icon_solid("folder")
            .size((12.0 * scale).max(9.0))
            .color(FOLDER_ICON),
        text("References").size((12.0 * scale).max(9.0)).color(TEXT),
        Some(Message::SolutionTreeToggle(node_id.to_string())),
        None,
        scale,
    ));

    if is_expanded {
        for reference in references {
            rows.push(tree_row(
                depth + 1,
                false,
                false,
                fa_icon_solid("cube")
                    .size((11.0 * scale).max(8.0))
                    .color(REFERENCE_COLOR),
                text(&reference.name)
                    .size((12.0 * scale).max(9.0))
                    .color(TEXT),
                None,
                Some(Message::WorkspaceFileActivated(reference.path.clone())),
                scale,
            ));
        }
    }

    rows
}

fn render_file_node<'a>(
    state: &'a EditorState,
    node: &'a SolutionTreeNode,
    depth: usize,
    scale: f32,
) -> Vec<Element<'a, Message>> {
    let mut rows: Vec<Element<'a, Message>> = Vec::new();

    if node.is_directory {
        let node_id = format!("dir:{}", node.path.as_deref().unwrap_or(&node.name));
        let is_expanded = state.is_solution_node_expanded(&node_id);

        rows.push(tree_row(
            depth,
            is_expanded,
            !node.children.is_empty(),
            fa_icon_solid("folder")
                .size((12.0 * scale).max(9.0))
                .color(FOLDER_ICON),
            text(&node.name).size((12.0 * scale).max(9.0)).color(TEXT),
            Some(Message::SolutionTreeToggle(node_id)),
            None,
            scale,
        ));

        if is_expanded {
            for child in &node.children {
                rows.extend(render_file_node(state, child, depth + 1, scale));
            }
        }
    } else {
        // File node
        let (icon_name, icon_color) = file_icon(&node.name);

        let click_msg = node
            .path
            .as_ref()
            .map(|p| Message::WorkspaceFileActivated(p.clone()));

        rows.push(tree_row(
            depth,
            false,
            false,
            fa_icon_solid(icon_name)
                .size((11.0 * scale).max(8.0))
                .color(icon_color),
            text(&node.name).size((12.0 * scale).max(9.0)).color(TEXT),
            None,
            click_msg,
            scale,
        ));
    }

    rows
}

// ============================================================================
// Makefile Rendering
// ============================================================================

fn render_makefile<'a>(
    state: &'a EditorState,
    makefile: &'a MakefileEntry,
    scale: f32,
) -> Element<'a, Message> {
    let node_id = format!("make:{}", makefile.path);
    let is_expanded = state.is_solution_node_expanded(&node_id);

    let mut rows: Vec<Element<'a, Message>> = Vec::new();

    rows.push(tree_row(
        0,
        is_expanded,
        !makefile.files.is_empty(),
        fa_icon_solid("cog")
            .size((12.0 * scale).max(9.0))
            .color(FOLDER_ICON),
        text(&makefile.name)
            .size((13.0 * scale).max(10.0))
            .color(TEXT),
        Some(Message::SolutionTreeToggle(node_id)),
        Some(Message::WorkspaceFileActivated(makefile.path.clone())),
        scale,
    ));

    if is_expanded {
        for file in &makefile.files {
            rows.extend(render_file_node(state, file, 1, scale));
        }
    }

    Column::with_children(rows).spacing(0.0).into()
}

// ============================================================================
// Error Rendering
// ============================================================================

fn render_error<'a>(error: &'a SolutionErrorEntry, scale: f32) -> Element<'a, Message> {
    row![
        fa_icon_solid("circle-exclamation")
            .size((12.0 * scale).max(9.0))
            .color(ERROR),
        text(format!("{}: {}", error.path, error.message))
            .size((12.0 * scale).max(9.0))
            .color(ERROR),
    ]
    .spacing(6.0)
    .padding(Padding::from([4.0, 12.0]))
    .align_y(Alignment::Center)
    .into()
}

// ============================================================================
// Tree Row Helper
// ============================================================================

fn tree_row<'a>(
    depth: usize,
    is_expanded: bool,
    has_children: bool,
    icon: impl Into<Element<'a, Message>>,
    label: iced::widget::Text<'a>,
    toggle_msg: Option<Message>,
    click_msg: Option<Message>,
    scale: f32,
) -> Element<'a, Message> {
    let indent = Space::new().width(INDENT_PX * depth as f32);

    // Chevron for expand/collapse
    let chevron: Element<'a, Message> = if has_children {
        let chevron_icon = if is_expanded {
            fa_icon_solid("chevron-down")
        } else {
            fa_icon_solid("chevron-right")
        };

        let chevron_btn = button(
            chevron_icon
                .size((10.0 * scale).max(7.0))
                .color(CHEVRON_COLOR),
        )
        .style(style::chevron_button())
        .padding(Padding::from([2, 4]));

        if let Some(msg) = toggle_msg {
            chevron_btn.on_press(msg).into()
        } else {
            chevron_btn.into()
        }
    } else {
        // Placeholder for alignment
        Space::new().width(18.0).into()
    };

    // Build the row content
    let content: Row<'a, Message> =
        row![indent, chevron, icon.into(), Space::new().width(4.0), label,]
            .spacing(2.0)
            .align_y(Alignment::Center);

    // Wrap in button if clickable
    if let Some(msg) = click_msg {
        button(content)
            .style(style::tree_row_button(false))
            .padding(Padding::from([2, 8]))
            .width(Length::Fill)
            .on_press(msg)
            .into()
    } else {
        content
            .padding(Padding::from([2, 8]))
            .height(ROW_HEIGHT * scale)
            .into()
    }
}

// ============================================================================
// File Icon Helper
// ============================================================================

/// Get icon name and color for a file based on its extension
fn file_icon(name: &str) -> (&'static str, iced::Color) {
    let ext = name
        .rsplit('.')
        .next()
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        // C/C++ source
        "c" | "cpp" | "cxx" | "cc" | "c++" => ("file-code", iced::Color::from_rgb(0.3, 0.6, 0.9)),
        // Headers
        "h" | "hpp" | "hxx" | "hh" | "h++" => ("file-lines", iced::Color::from_rgb(0.6, 0.4, 0.8)),
        // Build files
        "vcxproj" | "sln" | "cmake" | "makefile" => {
            ("file-invoice", iced::Color::from_rgb(0.9, 0.7, 0.3))
        }
        // Config/data
        "json" | "xml" | "yaml" | "yml" | "toml" => {
            ("file-code", iced::Color::from_rgb(0.5, 0.7, 0.5))
        }
        // Resources
        "rc" | "ico" | "bmp" | "png" | "jpg" | "jpeg" => {
            ("file-image", iced::Color::from_rgb(0.8, 0.5, 0.6))
        }
        // Text/docs
        "txt" | "md" | "readme" => ("file-lines", FILE_ICON),
        // Default
        _ => ("file", FILE_ICON),
    }
}
