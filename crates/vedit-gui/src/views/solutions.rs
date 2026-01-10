use crate::message::Message;
use crate::state::{
    EditorState, MakefileEntry, SolutionBrowserEntry, SolutionErrorEntry, SolutionTreeNode,
    VisualStudioProjectEntry, VisualStudioSolutionEntry,
};
use crate::style::{document_button, TEXT, MUTED, ERROR, WARNING};
use iced::widget::{button, column, Space, row, text, Column};
use iced::{Alignment, Element, Length, Padding};

pub fn render_solutions_tab(state: &EditorState, scale: f32) -> Column<'_, Message> {
    let mut content = column![
        text("Solutions")
            .size((16.0 * scale).max(12.0))
            .color(TEXT)
    ]
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
    let mut content = column![
        button(
            text(format!("🟦 {}", solution.name))
                .color(TEXT)
                .size((14.0 * scale).max(10.0)),
        )
        .style(document_button())
        .on_press(Message::SolutionSelected(solution.path.clone()))
    ]
    .spacing(spacing);

    for warning in &solution.warnings {
        content = content.push(
            row![
                Space::new().width(Length::Fill).width(Length::Fixed(16.0)),
                text(warning)
                    .color(WARNING)
                    .size((12.0 * scale).max(9.0)),
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
    let mut column = Column::new().spacing(spacing);

    let header = row![
        Space::new().width(Length::Fill).width(Length::Fixed(16.0)),
        text("🛠").size((13.0 * scale).max(9.0)),
        text(&project.name)
            .color(TEXT)
            .size((13.0 * scale).max(9.0)),
    ]
    .spacing(spacing)
    .align_y(Alignment::Center);

    column = column.push(
        button(header)
            .style(document_button())
            .on_press(Message::WorkspaceFileActivated(project.path.clone())),
    );

    if let Some(error) = &project.load_error {
        column = column.push(
            row![
                Space::new().width(Length::Fill).width(Length::Fixed(32.0)),
                text(error)
                    .color(ERROR)
                    .size((12.0 * scale).max(9.0)),
            ]
            .spacing(spacing)
            .align_y(Alignment::Center),
        );
    } else if !project.files.is_empty() {
        column = column.push(render_solution_node_column(&project.files, 32.0, scale));
    }

    column.into()
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
            Space::new().width(Length::Fill).width(Length::Fixed(indent)),
            text(icon).size((12.0 * scale).max(9.0)),
            text(&node.name)
                .color(TEXT)
                .size((13.0 * scale).max(9.0)),
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
            column = column.push(render_solution_node_column(&node.children, indent + 16.0, scale));
        }
    }

    column
}