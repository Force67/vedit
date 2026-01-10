use crate::debugger::{DebuggerConsoleEntryKind, DebuggerState, DebuggerType};
use crate::message::Message;
use crate::style::panel_container;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, checkbox, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length};

pub fn menu<'a>(
    debugger: &'a DebuggerState,
    scale: f32,
    spacing_large: f32,
    spacing_medium: f32,
    spacing_small: f32,
) -> Element<'a, Message> {
    let mut target_entries = column![].spacing(spacing_small);

    for target in debugger.filtered_targets() {
        let id = target.id;
        let label = format!("{} - {}", target.name, target.source);
        let entry = checkbox(debugger.is_target_selected(id))
            .label(label)
            .size((14.0 * scale).max(10.0))
            .on_toggle(move |selected| Message::DebuggerTargetToggled(id, selected));
        target_entries = target_entries.push(entry);
    }

    let target_list = scrollable(target_entries)
        .height(Length::Fixed((280.0 * scale).max(200.0)))
        .width(Length::Fill);

    let filter_input = text_input("Search targets", debugger.target_filter())
        .on_input(Message::DebuggerTargetFilterChanged)
        .padding((6.0 * scale).max(4.0))
        .size((14.0 * scale).max(10.0))
        .width(Length::Fill);

    let target_controls = row![
        button("Refresh Targets")
            .on_press(Message::DebuggerTargetsRefreshRequested)
            .padding((6.0 * scale).max(4.0)),
    ]
    .spacing(spacing_small)
    .align_y(Alignment::Center);

    let selected_count = debugger.selected_target_count();

    let target_details = debugger
        .primary_selected_target()
        .map(|target| {
            let mut details = column![
                text(format!("Executable: {}", target.executable.display()))
                    .size((14.0 * scale).max(10.0)),
                text(format!(
                    "Working dir: {}",
                    target.working_directory.display()
                ))
                .size((14.0 * scale).max(10.0)),
                text(format!("Arguments: {}", target.args.join(" ")))
                    .size((14.0 * scale).max(10.0)),
                text(format!("Source: {}", target.source)).size((14.0 * scale).max(10.0)),
            ]
            .spacing((4.0 * scale).max(2.0));

            if let Some(notes) = &target.notes {
                details = details.push(
                    text(notes)
                        .size((13.0 * scale).max(9.0))
                        .color(iced::Color::from_rgb8(180, 180, 180)),
                );
            }

            details
        })
        .unwrap_or_else(|| {
            column![
                text("No target selected")
                    .size((14.0 * scale).max(10.0))
                    .color(iced::Color::from_rgb8(200, 200, 200))
            ]
        });

    let status_line =
        text(format!("Status: {}", debugger.status().label())).size((14.0 * scale).max(10.0));

    let targets_section = column![
        text("Debug Targets").size((16.0 * scale).max(12.0)),
        target_controls,
        filter_input,
        target_list,
        text(format!("Selected targets: {}", selected_count))
            .size((13.0 * scale).max(9.0))
            .color(iced::Color::from_rgb8(180, 180, 180)),
        target_details,
        status_line,
    ]
    .spacing(spacing_small);

    let mut breakpoints_list = column![].spacing(spacing_small);

    for breakpoint in debugger.breakpoints() {
        let id = breakpoint.id;
        let checkbox = checkbox(breakpoint.enabled)
            .label(format!(
                "{}:{}",
                breakpoint.display_path(debugger.workspace_root()),
                breakpoint.line
            ))
            .size((14.0 * scale).max(10.0))
            .on_toggle(move |_| Message::DebuggerBreakpointToggled(id));

        let condition_value = breakpoint.condition.as_deref().unwrap_or("");
        let condition = text_input("Condition", condition_value)
            .on_input(move |value| Message::DebuggerBreakpointConditionChanged(id, value))
            .padding((6.0 * scale).max(4.0))
            .size((14.0 * scale).max(10.0))
            .width(Length::Fill);

        let remove_button = button("Remove")
            .on_press(Message::DebuggerBreakpointRemoved(id))
            .padding((6.0 * scale).max(4.0));

        let row = row![checkbox, condition, remove_button,]
            .spacing(spacing_small)
            .align_y(Alignment::Center);
        breakpoints_list = breakpoints_list.push(row);
    }

    let breakpoint_form = {
        let draft = debugger.breakpoint_draft();
        let file_input = text_input("File path", draft.file.as_str())
            .on_input(Message::DebuggerBreakpointDraftFileChanged)
            .padding((6.0 * scale).max(4.0))
            .size((14.0 * scale).max(10.0))
            .width(Length::Fill);

        let line_input = text_input("Line", draft.line.as_str())
            .on_input(Message::DebuggerBreakpointDraftLineChanged)
            .padding((6.0 * scale).max(4.0))
            .size((14.0 * scale).max(10.0))
            .width(Length::Fixed((70.0 * scale).max(48.0)));

        let condition_input = text_input("Condition (optional)", draft.condition.as_str())
            .on_input(Message::DebuggerBreakpointDraftConditionChanged)
            .padding((6.0 * scale).max(4.0))
            .size((14.0 * scale).max(10.0))
            .width(Length::Fill);

        let add_button = button("Add Breakpoint")
            .on_press(Message::DebuggerBreakpointDraftSubmitted)
            .padding((6.0 * scale).max(4.0));

        column![
            row![file_input, line_input]
                .spacing(spacing_small)
                .align_y(Alignment::Center),
            row![condition_input, add_button]
                .spacing(spacing_small)
                .align_y(Alignment::Center),
        ]
        .spacing(spacing_small)
    };

    let breakpoints_section = column![
        text("Breakpoints").size((16.0 * scale).max(12.0)),
        scrollable(breakpoints_list)
            .height(Length::Fixed((160.0 * scale).max(120.0)))
            .width(Length::Fill),
        breakpoint_form,
    ]
    .spacing(spacing_small);

    let manual_target_form = {
        let draft = debugger.manual_target_draft();
        let name_input = text_input("Target name", draft.name.as_str())
            .on_input(Message::DebuggerManualTargetNameChanged)
            .padding((6.0 * scale).max(4.0))
            .size((14.0 * scale).max(10.0))
            .width(Length::Fill);
        let executable_input = text_input("Executable", draft.executable.as_str())
            .on_input(Message::DebuggerManualTargetExecutableChanged)
            .padding((6.0 * scale).max(4.0))
            .size((14.0 * scale).max(10.0))
            .width(Length::Fill);
        let working_dir_input = text_input("Working directory", draft.working_directory.as_str())
            .on_input(Message::DebuggerManualTargetWorkingDirectoryChanged)
            .padding((6.0 * scale).max(4.0))
            .size((14.0 * scale).max(10.0))
            .width(Length::Fill);
        let args_input = text_input("Arguments", draft.arguments.as_str())
            .on_input(Message::DebuggerManualTargetArgumentsChanged)
            .padding((6.0 * scale).max(4.0))
            .size((14.0 * scale).max(10.0))
            .width(Length::Fill);
        let save_button = button("Save Target")
            .on_press(Message::DebuggerManualTargetSaved)
            .padding((6.0 * scale).max(4.0));

        column![
            name_input,
            executable_input,
            working_dir_input,
            args_input,
            save_button,
        ]
        .spacing(spacing_small)
    };

    let gdb_script_input = text_input("Initial gdb commands", debugger.launch_script())
        .on_input(Message::DebuggerLaunchScriptChanged)
        .padding((6.0 * scale).max(4.0))
        .size((14.0 * scale).max(10.0))
        .width(Length::Fill);

    let console_entries = debugger.console().iter().fold(column![], |column, entry| {
        let color = match entry.kind {
            DebuggerConsoleEntryKind::Command => iced::Color::from_rgb8(180, 200, 250),
            DebuggerConsoleEntryKind::Output => iced::Color::from_rgb8(200, 200, 200),
            DebuggerConsoleEntryKind::Error => iced::Color::from_rgb8(240, 128, 128),
            DebuggerConsoleEntryKind::Info => iced::Color::from_rgb8(180, 220, 180),
        };
        column.push(
            text(&entry.message)
                .size((13.0 * scale).max(9.0))
                .color(color),
        )
    });

    let command_input = text_input("command", debugger.command_input())
        .on_input(Message::DebuggerGdbCommandInputChanged)
        .on_submit(Message::DebuggerGdbCommandSubmitted)
        .padding((6.0 * scale).max(4.0))
        .size((14.0 * scale).max(10.0))
        .width(Length::Fill);

    let send_button = button("Send")
        .on_press(Message::DebuggerGdbCommandSubmitted)
        .padding((6.0 * scale).max(4.0));

    let console_section = column![
        text("gdb Console").size((16.0 * scale).max(12.0)),
        scrollable(console_entries)
            .height(Length::Fixed((140.0 * scale).max(120.0)))
            .width(Length::Fill),
        row![command_input, send_button]
            .spacing(spacing_small)
            .align_y(Alignment::Center),
    ]
    .spacing(spacing_small);

    let debugger_type_selector = row![
        text("Debugger Type:").size((14.0 * scale).max(10.0)),
        iced::widget::radio(
            "GDB",
            DebuggerType::Gdb,
            Some(debugger.debugger_type()),
            |dt| Message::DebuggerTypeChanged(dt)
        ),
        iced::widget::radio(
            "Vedit",
            DebuggerType::Vedit,
            Some(debugger.debugger_type()),
            |dt| Message::DebuggerTypeChanged(dt)
        ),
    ]
    .spacing(spacing_small)
    .align_y(Alignment::Center);

    let debugger_title = match debugger.debugger_type() {
        DebuggerType::Gdb => "Debugger (gdb)",
        DebuggerType::Vedit => "Debugger (vedit)",
    };

    let layout = column![
        text(debugger_title)
            .size((18.0 * scale).max(14.0))
            .align_x(Horizontal::Left),
        debugger_type_selector,
        targets_section,
        breakpoints_section,
        manual_target_form,
        gdb_script_input,
        console_section,
    ]
    .spacing(spacing_medium);

    container(layout)
        .padding(spacing_large)
        .width(Length::Fill)
        .max_width((640.0 * scale).max(360.0))
        .style(panel_container())
        .align_x(Horizontal::Left)
        .align_y(Vertical::Top)
        .into()
}
