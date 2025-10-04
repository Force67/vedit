use iced::widget::text_editor::{Action as TextEditorAction, Content};
use vedit_core::Editor;

#[derive(Debug)]
pub struct EditorState {
    editor: Editor,
    error: Option<String>,
    buffer_content: Content,
}

impl Default for EditorState {
    fn default() -> Self {
        let mut state = Self {
            editor: Editor::new(),
            error: None,
            buffer_content: Content::new(),
        };
        state.sync_buffer_from_editor();
        state
    }
}

impl EditorState {
    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    pub fn editor_mut(&mut self) -> &mut Editor {
        &mut self.editor
    }

    pub fn buffer_content(&self) -> &Content {
        &self.buffer_content
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn set_error(&mut self, message: Option<String>) {
        self.error = message;
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    pub fn sync_buffer_from_editor(&mut self) {
        let contents = self
            .editor
            .active_document()
            .map(|doc| doc.buffer.clone())
            .unwrap_or_default();

        self.buffer_content = Content::with_text(&contents);
    }

    pub fn apply_buffer_action(&mut self, action: TextEditorAction) {
        let is_edit = action.is_edit();
        self.buffer_content.perform(action);

        if is_edit {
            let updated = self.editor_contents_to_string();
            self.editor.update_active_buffer(updated);
        }
    }

    fn editor_contents_to_string(&self) -> String {
        let mut text = self.buffer_content.text();
        if text.ends_with('\n') {
            text.pop();
        }
        text
    }
}
