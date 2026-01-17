use crate::message::Message;
use crate::state::EditorState;
use crate::style;
use iced::widget::{Space, button, container, row, scrollable, text};
use iced::{Alignment, Element, Length, Padding};
use iced_font_awesome::{fa_icon_brands, fa_icon_solid};

/// Renders the document tab bar at the top of the editor
pub fn render_document_tabs(state: &EditorState, scale: f32) -> Element<'_, Message> {
    let documents = state.editor().open_documents();
    let active_index = state.editor().active_index();

    let mut tabs_row = row![].spacing(0).align_y(Alignment::Center);

    for (index, document) in documents.iter().enumerate() {
        let is_active = index == active_index;
        let mut title = document.display_name().to_string();
        if document.is_modified {
            title.push_str(" •");
        }

        // File icon based on extension
        let (icon, is_brand) = get_file_icon(&title);
        let icon_color = if is_active {
            style::TEXT
        } else {
            style::FILE_ICON
        };

        let icon_element: Element<'_, Message> = if is_brand {
            fa_icon_brands(icon).size(11.0).color(icon_color).into()
        } else {
            fa_icon_solid(icon).size(11.0).color(icon_color).into()
        };

        let title_text = text(title)
            .size((12.0 * scale).max(10.0))
            .color(if is_active {
                style::TEXT
            } else {
                style::TEXT_SECONDARY
            });

        // Close button - visible on hover (always visible for now)
        let close_btn = button(fa_icon_solid("xmark").size(9.0).color(style::MUTED))
            .style(style::tab_close_button())
            .padding(Padding::from([2, 4]))
            .on_press(Message::CloseDocument(index));

        let tab_content = row![icon_element, title_text, close_btn]
            .spacing(6)
            .align_y(Alignment::Center);

        let tab_button = button(tab_content)
            .style(style::document_tab(is_active))
            .padding(Padding::from([6, 12]))
            .on_press(Message::DocumentSelected(index));

        tabs_row = tabs_row.push(tab_button);
    }

    // Add spacer to fill remaining width
    tabs_row = tabs_row.push(Space::new().width(Length::Fill));

    let tabs_container = container(
        scrollable(tabs_row)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new().width(0).scroller_width(0),
            ))
            .style(style::invisible_scrollable()),
    )
    .padding(Padding::from([0, 4]))
    .width(Length::Fill)
    .style(style::tab_bar_container());

    tabs_container.into()
}

/// Get appropriate icon for file type
/// Returns (icon_name, is_brand_icon)
fn get_file_icon(filename: &str) -> (&'static str, bool) {
    let filename_lower = filename.to_lowercase();

    if filename_lower.ends_with(".rs") {
        ("rust", true)
    } else if filename_lower.ends_with(".py") {
        ("python", true)
    } else if filename_lower.ends_with(".js") || filename_lower.ends_with(".jsx") {
        ("js", true)
    } else if filename_lower.ends_with(".ts") || filename_lower.ends_with(".tsx") {
        ("js", true) // TypeScript uses JS icon (no dedicated TS brand icon)
    } else if filename_lower.ends_with(".html") || filename_lower.ends_with(".htm") {
        ("html5", true)
    } else if filename_lower.ends_with(".css") || filename_lower.ends_with(".scss") {
        ("css3", true)
    } else if filename_lower.ends_with(".json") {
        ("brackets-curly", false)
    } else if filename_lower.ends_with(".md") || filename_lower.ends_with(".markdown") {
        ("markdown", true)
    } else if filename_lower.ends_with(".toml")
        || filename_lower.ends_with(".yaml")
        || filename_lower.ends_with(".yml")
    {
        ("gear", false)
    } else if filename_lower.ends_with(".c") || filename_lower.ends_with(".h") {
        ("c", false) // solid "c" icon
    } else if filename_lower.ends_with(".cpp")
        || filename_lower.ends_with(".hpp")
        || filename_lower.ends_with(".cc")
    {
        ("code", false)
    } else if filename_lower.ends_with(".go") {
        ("golang", true)
    } else if filename_lower.ends_with(".lua") {
        ("moon", false)
    } else if filename_lower.ends_with(".nix") {
        ("snowflake", false)
    } else if filename_lower.ends_with(".sh") || filename_lower.ends_with(".bash") {
        ("terminal", false)
    } else if filename_lower.ends_with(".sql") {
        ("database", false)
    } else if filename_lower.ends_with(".xml") {
        ("code", false)
    } else if filename_lower.ends_with(".txt") {
        ("file-lines", false)
    } else if filename_lower.contains("makefile") || filename_lower.contains("cmake") {
        ("hammer", false)
    } else if filename_lower.contains("dockerfile") {
        ("docker", true)
    } else if filename_lower.contains("gitignore") || filename_lower.contains("git") {
        ("git-alt", true) // brand icon for git
    } else {
        ("file", false)
    }
}
