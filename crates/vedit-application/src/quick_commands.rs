#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuickCommandId {
    OpenFile,
    OpenFolder,
    OpenSolution,
    SaveFile,
    NewScratchBuffer,
    ShowScaleFactor,
    AddStickyNote,
    IncreaseCodeFontZoom,
}

#[derive(Debug, Clone, Copy)]
pub struct QuickCommand {
    pub id: QuickCommandId,
    pub title: &'static str,
    pub description: &'static str,
    pub action: Option<&'static str>,
}

static QUICK_COMMANDS: &[QuickCommand] = &[
    QuickCommand {
        id: QuickCommandId::OpenFile,
        title: "Open File…",
        description: "Select a file from disk",
        action: Some("quick_command.open_file"),
    },
    QuickCommand {
        id: QuickCommandId::OpenFolder,
        title: "Open Folder…",
        description: "Choose a workspace directory",
        action: Some("quick_command.open_folder"),
    },
    QuickCommand {
        id: QuickCommandId::OpenSolution,
        title: "Open Solution…",
        description: "Select a Visual Studio solution",
        action: Some("quick_command.open_solution"),
    },
    QuickCommand {
        id: QuickCommandId::SaveFile,
        title: "Save File",
        description: "Write the current buffer to disk",
        action: Some("quick_command.save_file"),
    },
    QuickCommand {
        id: QuickCommandId::NewScratchBuffer,
        title: "New Scratch Buffer",
        description: "Create an empty buffer for quick notes",
        action: Some("quick_command.new_scratch"),
    },
    QuickCommand {
        id: QuickCommandId::ShowScaleFactor,
        title: "Show Detected Scale",
        description: "Log the current UI scale factor",
        action: None,
    },
    QuickCommand {
        id: QuickCommandId::AddStickyNote,
        title: "Add Sticky Note",
        description: "Attach a sticky note at the current cursor",
        action: Some("quick_command.add_sticky_note"),
    },
    QuickCommand {
        id: QuickCommandId::IncreaseCodeFontZoom,
        title: "Increase Code Font Zoom",
        description: "Make the code window font larger",
        action: Some("quick_command.increase_code_font_zoom"),
    },
];

pub fn list() -> &'static [QuickCommand] {
    QUICK_COMMANDS
}
