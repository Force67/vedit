use crate::message::Message;
use crate::state::{
    EditorState, MakefileEntry, SolutionBrowserEntry, SolutionErrorEntry, SolutionTreeNode,
    VisualStudioProjectEntry, VisualStudioSolutionEntry,
};
use crate::style::{ERROR, MUTED, TEXT, WARNING, document_button};
use iced::widget::{Column, Space, button, column, row, text};
use iced::{Alignment, Element, Length, Padding};

// Accent color for metadata
const ACCENT: iced::Color = iced::Color {
    r: 0.4,
    g: 0.6,
    b: 0.9,
    a: 1.0,
};

pub fn render_solutions_tab(state: &EditorState, scale: f32) -> Column<'_, Message> {
    let mut content = column![text("Solutions").size((16.0 * scale).max(12.0)).color(TEXT)]
        .spacing((6.0 * scale).max(3.0))
        .padding(Padding::from([8.0, 16.0]));

    let entries = state.workspace_solutions();
    if entries.is_empty() {
        content = content.push(
            text("No solutions or Makefiles found")
                .color(MUTED)
                .size((13.0 * scale).max(9.0)),
        );
        return content;
    }

    for entry in entries {
        content = content.push(render_solution_entry(entry, scale));
    }

    content
}

fn render_solution_entry(entry: &SolutionBrowserEntry, scale: f32) -> Element<'_, Message> {
    match entry {
        SolutionBrowserEntry::VisualStudio(solution) => {
            render_visual_studio_solution(solution, scale)
        }
        SolutionBrowserEntry::Makefile(makefile) => render_makefile_entry(makefile, scale),
        SolutionBrowserEntry::Error(error) => render_solution_error(error, scale),
    }
}

fn render_visual_studio_solution(
    solution: &VisualStudioSolutionEntry,
    scale: f32,
) -> Element<'_, Message> {
    let spacing = (4.0 * scale).max(2.0);

    // Build solution header with version info
    let header_text = if let Some(ref vs_version) = solution.vs_version {
        // Extract major version number
        let version_short = vs_version.split('.').next().unwrap_or(vs_version);
        format!("🟦 {} (VS {})", solution.name, version_short)
    } else {
        format!("🟦 {}", solution.name)
    };

    let mut content = column![
        button(
            text(header_text)
                .color(TEXT)
                .size((14.0 * scale).max(10.0)),
        )
        .style(document_button())
        .on_press(Message::SolutionSelected(solution.path.clone()))
    ]
    .spacing(spacing);

    // Show configurations if available
    if !solution.configurations.is_empty() {
        let configs_str = solution.configurations.join(", ");
        content = content.push(
            row![
                Space::new().width(Length::Fixed(16.0)),
                text(format!("Configs: {}", configs_str))
                    .color(MUTED)
                    .size((11.0 * scale).max(8.0)),
            ]
            .spacing(spacing)
            .align_y(Alignment::Center),
        );
    }

    // Show solution folders if any
    for folder in &solution.folders {
        if !folder.project_names.is_empty() {
            content = content.push(
                row![
                    Space::new().width(Length::Fixed(16.0)),
                    text(format!("📁 {} ({})", folder.name, folder.project_names.len()))
                        .color(ACCENT)
                        .size((12.0 * scale).max(9.0)),
                ]
                .spacing(spacing)
                .align_y(Alignment::Center),
            );
        }
    }

    for warning in &solution.warnings {
        content = content.push(
            row![
                Space::new().width(Length::Fixed(16.0)),
                text(warning).color(WARNING).size((12.0 * scale).max(9.0)),
            ]
            .spacing(spacing)
            .align_y(Alignment::Center),
        );
    }

    for project in &solution.projects {
        content = content.push(render_visual_studio_project(project, scale));
    }

    content.into()
}

fn render_visual_studio_project(
    project: &VisualStudioProjectEntry,
    scale: f32,
) -> Element<'_, Message> {
    let spacing = (3.0 * scale).max(2.0);
    let mut col = Column::new().spacing(spacing);

    // Build project header with type icon
    let type_icon = match project.project_type.as_deref() {
        Some("Application") => "🎯",
        Some("Dynamic Library") => "📦",
        Some("Static Library") => "📚",
        _ => "🛠",
    };

    // Build metadata suffix
    let mut meta_parts = Vec::new();
    if let Some(ref toolset) = project.platform_toolset {
        meta_parts.push(toolset.clone());
    }
    if let Some(ref proj_type) = project.project_type {
        if proj_type != "Application" {
            meta_parts.push(proj_type.clone());
        }
    }
    let meta_suffix = if meta_parts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", meta_parts.join(", "))
    };

    let header = row![
        Space::new().width(Length::Fixed(16.0)),
        text(type_icon).size((13.0 * scale).max(9.0)),
        text(format!("{}{}", project.name, meta_suffix))
            .color(TEXT)
            .size((13.0 * scale).max(9.0)),
    ]
    .spacing(spacing)
    .align_y(Alignment::Center);

    col = col.push(
        button(header)
            .style(document_button())
            .on_press(Message::WorkspaceFileActivated(project.path.clone())),
    );

    if let Some(error) = &project.load_error {
        col = col.push(
            row![
                Space::new().width(Length::Fixed(32.0)),
                text(error).color(ERROR).size((12.0 * scale).max(9.0)),
            ]
            .spacing(spacing)
            .align_y(Alignment::Center),
        );
    } else {
        // Show project references (dependencies)
        if !project.references.is_empty() {
            let refs_str = project
                .references
                .iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            col = col.push(
                row![
                    Space::new().width(Length::Fixed(32.0)),
                    text(format!("→ {}", refs_str))
                        .color(ACCENT)
                        .size((11.0 * scale).max(8.0)),
                ]
                .spacing(spacing)
                .align_y(Alignment::Center),
            );
        }

        // Show include directories (abbreviated)
        if !project.include_dirs.is_empty() {
            let includes_preview = if project.include_dirs.len() <= 3 {
                project.include_dirs.join(", ")
            } else {
                format!(
                    "{}, ... (+{})",
                    project.include_dirs[..2].join(", "),
                    project.include_dirs.len() - 2
                )
            };
            col = col.push(
                row![
                    Space::new().width(Length::Fixed(32.0)),
                    text(format!("Inc: {}", includes_preview))
                        .color(MUTED)
                        .size((10.0 * scale).max(8.0)),
                ]
                .spacing(spacing)
                .align_y(Alignment::Center),
            );
        }

        // Show preprocessor definitions (abbreviated)
        if !project.preprocessor_defs.is_empty() {
            let defs_preview = if project.preprocessor_defs.len() <= 3 {
                project.preprocessor_defs.join(", ")
            } else {
                format!(
                    "{}, ...",
                    project.preprocessor_defs[..3].join(", ")
                )
            };
            col = col.push(
                row![
                    Space::new().width(Length::Fixed(32.0)),
                    text(format!("Defs: {}", defs_preview))
                        .color(MUTED)
                        .size((10.0 * scale).max(8.0)),
                ]
                .spacing(spacing)
                .align_y(Alignment::Center),
            );
        }

        // Show files
        if !project.files.is_empty() {
            col = col.push(render_solution_node_column(&project.files, 32.0, scale));
        }
    }

    col.into()
}

fn render_makefile_entry(makefile: &MakefileEntry, scale: f32) -> Element<'_, Message> {
    let spacing = (4.0 * scale).max(2.0);
    let mut column = column![
        button(
            text(format!("⚙ {}", makefile.name))
                .color(TEXT)
                .size((14.0 * scale).max(10.0)),
        )
        .style(document_button())
        .on_press(Message::WorkspaceFileActivated(makefile.path.clone()))
    ]
    .spacing(spacing);

    if makefile.files.is_empty() {
        column = column.push(
            row![
                Space::new().width(Length::Fill).width(Length::Fixed(16.0)),
                text("No referenced files detected")
                    .color(MUTED)
                    .size((12.0 * scale).max(9.0)),
            ]
            .spacing(spacing)
            .align_y(Alignment::Center),
        );
    } else {
        column = column.push(render_solution_node_column(&makefile.files, 16.0, scale));
    }

    column.into()
}

fn render_solution_error(error: &SolutionErrorEntry, scale: f32) -> Element<'_, Message> {
    column![
        text(format!("{}: {}", error.path, error.message))
            .color(ERROR)
            .size((12.0 * scale).max(9.0)),
    ]
    .spacing((2.0 * scale).max(1.0))
    .padding(Padding::from([4.0, 16.0]))
    .into()
}

fn render_solution_node_column<'a>(
    nodes: &'a [SolutionTreeNode],
    indent: f32,
    scale: f32,
) -> Column<'a, Message> {
    let spacing = (3.0 * scale).max(1.0);
    let mut column = Column::new().spacing(spacing);

    for node in nodes {
        let icon = if node.is_directory { "📁" } else { "📄" };
        let row_content = row![
            Space::new()
                .width(Length::Fill)
                .width(Length::Fixed(indent)),
            text(icon).size((12.0 * scale).max(9.0)),
            text(&node.name).color(TEXT).size((13.0 * scale).max(9.0)),
        ]
        .spacing(spacing)
        .align_y(Alignment::Center);

        let element: Element<'_, Message> = if let Some(path) = &node.path {
            button(row_content)
                .style(document_button())
                .on_press(Message::WorkspaceFileActivated(path.clone()))
                .into()
        } else {
            row_content.into()
        };

        column = column.push(element);

        if !node.children.is_empty() {
            column = column.push(render_solution_node_column(
                &node.children,
                indent + 16.0,
                scale,
            ));
        }
    }

    column
}
