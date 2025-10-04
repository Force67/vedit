#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickCommandId {
    OpenFile,
    OpenFolder,
    NewScratchBuffer,
}

#[derive(Debug, Clone, Copy)]
pub struct QuickCommand {
    pub id: QuickCommandId,
    pub title: &'static str,
    pub description: &'static str,
}

static QUICK_COMMANDS: &[QuickCommand] = &[
    QuickCommand {
        id: QuickCommandId::OpenFile,
        title: "Open File…",
        description: "Select a file from disk",
    },
    QuickCommand {
        id: QuickCommandId::OpenFolder,
        title: "Open Folder…",
        description: "Choose a workspace directory",
    },
    QuickCommand {
        id: QuickCommandId::NewScratchBuffer,
        title: "New Scratch Buffer",
        description: "Create an empty buffer for quick notes",
    },
];

pub fn commands() -> &'static [QuickCommand] {
    QUICK_COMMANDS
}
