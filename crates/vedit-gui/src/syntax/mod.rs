use iced::advanced::text::highlighter::{Format as HighlightFormat, Highlighter as IcedHighlighter};
use iced::Color;
use std::collections::HashMap;
use std::fmt;
use std::ops::Range;
use std::sync::{Arc, Mutex};
use tree_sitter::Language as TsLanguage;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter as TsHighlighter};
use vedit_core::Language;

/// Identifier that uniquely represents an open document for syntax highlighting purposes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DocumentKey {
    Fingerprint(u64),
    Index(usize),
}

/// Manages syntax highlighting data for all open documents.
pub struct SyntaxSystem {
    theme: Arc<SyntaxTheme>,
    registry: LanguageRegistry,
    store: Arc<HighlightStore>,
}

impl fmt::Debug for SyntaxSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxSystem").finish()
    }
}

impl SyntaxSystem {
    pub fn new() -> Self {
        let theme = Arc::new(SyntaxTheme::default());
        let registry = LanguageRegistry::with_theme(Arc::clone(&theme));
        Self {
            theme,
            registry,
            store: Arc::new(HighlightStore::default()),
        }
    }

    pub fn settings_for(&self, key: DocumentKey) -> SyntaxSettings {
        SyntaxSettings {
            key,
            store: Arc::clone(&self.store),
            theme: Arc::clone(&self.theme),
        }
    }

    pub fn update_document(&self, key: DocumentKey, language: Language, contents: &str) {
        let highlight = if let Some(config) = self.registry.resolve(language) {
            match highlight_document(contents, config) {
                Ok(lines) => DocumentHighlight::with_lines(lines),
                Err(_) => DocumentHighlight::plain(contents),
            }
        } else {
            DocumentHighlight::plain(contents)
        };

        self.store.set(key, highlight);
    }
}

#[derive(Clone)]
struct LanguageConfig {
    configuration: Arc<HighlightConfiguration>,
    palette_map: Vec<usize>,
}

impl LanguageConfig {
    fn highlight_id_to_palette(&self, id: usize) -> usize {
        self.palette_map
            .get(id)
            .copied()
            .unwrap_or(PaletteIndex::TEXT)
    }
}

struct LanguageRegistry {
    theme: Arc<SyntaxTheme>,
    entries: HashMap<Language, LanguageConfig>,
}

impl LanguageRegistry {
    fn with_theme(theme: Arc<SyntaxTheme>) -> Self {
        let mut registry = Self {
            theme,
            entries: HashMap::new(),
        };

        let theme_ref = &registry.theme;

        if let Some(config) = build_config(
            tree_sitter_rust::LANGUAGE.into(),
            "rust",
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            Some(tree_sitter_rust::INJECTIONS_QUERY),
            None,
            theme_ref,
        ) {
            registry.entries.insert(Language::Rust, config);
        }

        if let Some(config) = build_config(
            tree_sitter_c::LANGUAGE.into(),
            "c",
            tree_sitter_c::HIGHLIGHT_QUERY,
            None,
            None,
            theme_ref,
        ) {
            registry.entries.insert(Language::C, config.clone());
            registry.entries.insert(Language::CHeader, config);
        }

        if let Some(config) = build_config(
            tree_sitter_cpp::LANGUAGE.into(),
            "cpp",
            tree_sitter_cpp::HIGHLIGHT_QUERY,
            None,
            None,
            theme_ref,
        ) {
            registry.entries.insert(Language::Cpp, config.clone());
            registry.entries.insert(Language::CppHeader, config);
        }

        if let Some(config) = build_config(
            tree_sitter_javascript::LANGUAGE.into(),
            "javascript",
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            Some(tree_sitter_javascript::INJECTIONS_QUERY),
            Some(tree_sitter_javascript::LOCALS_QUERY),
            theme_ref,
        ) {
            registry.entries.insert(Language::JavaScript, config);
        }

        if let Some(config) = build_config(
            tree_sitter_javascript::LANGUAGE.into(),
            "jsx",
            tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
            Some(tree_sitter_javascript::INJECTIONS_QUERY),
            Some(tree_sitter_javascript::LOCALS_QUERY),
            theme_ref,
        ) {
            registry.entries.insert(Language::Jsx, config);
        }

        if let Some(config) = build_config(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "typescript",
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
            None,
            Some(tree_sitter_typescript::LOCALS_QUERY),
            theme_ref,
        ) {
            registry.entries.insert(Language::TypeScript, config);
        }

        if let Some(config) = build_config(
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            "tsx",
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
            None,
            Some(tree_sitter_typescript::LOCALS_QUERY),
            theme_ref,
        ) {
            registry.entries.insert(Language::Tsx, config);
        }

        if let Some(config) = build_config(
            tree_sitter_python::LANGUAGE.into(),
            "python",
            tree_sitter_python::HIGHLIGHTS_QUERY,
            None,
            None,
            theme_ref,
        ) {
            registry.entries.insert(Language::Python, config);
        }

        if let Some(config) = build_config(
            tree_sitter_go::LANGUAGE.into(),
            "go",
            tree_sitter_go::HIGHLIGHTS_QUERY,
            None,
            None,
            theme_ref,
        ) {
            registry.entries.insert(Language::Go, config);
        }

        if let Some(config) = build_config(
            tree_sitter_json::LANGUAGE.into(),
            "json",
            tree_sitter_json::HIGHLIGHTS_QUERY,
            None,
            None,
            theme_ref,
        ) {
            registry.entries.insert(Language::Json, config);
        }

        if let Some(config) = build_config(
            tree_sitter_yaml::LANGUAGE.into(),
            "yaml",
            tree_sitter_yaml::HIGHLIGHTS_QUERY,
            None,
            None,
            theme_ref,
        ) {
            registry.entries.insert(Language::Yaml, config);
        }

        if let Some(config) = build_config(
            tree_sitter_html::LANGUAGE.into(),
            "html",
            tree_sitter_html::HIGHLIGHTS_QUERY,
            Some(tree_sitter_html::INJECTIONS_QUERY),
            None,
            theme_ref,
        ) {
            registry.entries.insert(Language::Html, config);
        }

        if let Some(config) = build_config(
            tree_sitter_css::LANGUAGE.into(),
            "css",
            tree_sitter_css::HIGHLIGHTS_QUERY,
            None,
            None,
            theme_ref,
        ) {
            registry.entries.insert(Language::Css, config);
        }

        registry
    }

    fn resolve(&self, language: Language) -> Option<&LanguageConfig> {
        self.entries.get(&language)
    }
}

fn build_config(
    language: TsLanguage,
    name: &str,
    highlights: &'static str,
    injections: Option<&'static str>,
    locals: Option<&'static str>,
    theme: &SyntaxTheme,
) -> Option<LanguageConfig> {
    let mut configuration = HighlightConfiguration::new(
        language,
        format!("vedit::{name}"),
        highlights,
        injections.unwrap_or(""),
        locals.unwrap_or(""),
    )
    .ok()?;

    configuration.configure(HIGHLIGHT_NAMES);

    let palette_map = HIGHLIGHT_NAMES
        .iter()
        .enumerate()
        .map(|(index, name)| theme.palette_index(name, index))
        .collect();

    Some(LanguageConfig {
        configuration: Arc::new(configuration),
        palette_map,
    })
}

const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "boolean",
    "comment",
    "comment.documentation",
    "constant",
    "constant.builtin",
    "constant.numeric",
    "constant.character",
    "constructor",
    "embedded",
    "escape",
    "function",
    "function.builtin",
    "function.macro",
    "function.method",
    "keyword",
    "keyword.control",
    "keyword.operator",
    "keyword.return",
    "keyword.function",
    "label",
    "method",
    "module",
    "number",
    "operator",
    "parameter",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation.special",
    "string",
    "string.regexp",
    "string.special",
    "symbol",
    "tag",
    "type",
    "type.builtin",
    "type.qualifier",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "variable.member",
    "variable.other",
    "variable.special",
    "variable.this",
    "markup.heading",
    "markup.list",
    "markup.bold",
    "markup.italic",
];

#[derive(Clone)]
struct SyntaxTheme {
    palette: Vec<Option<Color>>,
}

impl SyntaxTheme {
    fn default() -> Self {
        let mut palette = vec![None; PaletteIndex::TOTAL];

        palette[PaletteIndex::TEXT] = None;
        palette[PaletteIndex::COMMENT] = Some(Color::from_rgb8(117, 113, 94));
        palette[PaletteIndex::KEYWORD] = Some(Color::from_rgb8(197, 134, 192));
        palette[PaletteIndex::FUNCTION] = Some(Color::from_rgb8(130, 170, 255));
        palette[PaletteIndex::TYPE] = Some(Color::from_rgb8(224, 109, 117));
        palette[PaletteIndex::STRING] = Some(Color::from_rgb8(152, 195, 121));
        palette[PaletteIndex::NUMBER] = Some(Color::from_rgb8(209, 154, 102));
        palette[PaletteIndex::OPERATOR] = Some(Color::from_rgb8(86, 182, 194));
        palette[PaletteIndex::PROPERTY] = Some(Color::from_rgb8(224, 175, 104));
        palette[PaletteIndex::MACRO] = Some(Color::from_rgb8(198, 120, 221));
        palette[PaletteIndex::TAG] = Some(Color::from_rgb8(220, 120, 170));
        palette[PaletteIndex::ATTRIBUTE] = Some(Color::from_rgb8(190, 214, 255));
        palette[PaletteIndex::SPECIAL] = Some(Color::from_rgb8(97, 175, 239));
        palette[PaletteIndex::BOOLEAN] = Some(Color::from_rgb8(209, 154, 102));

        Self { palette }
    }

    fn palette_index(&self, name: &str, _idx: usize) -> usize {
        match name {
            "variable.member" | "variable.other" => return PaletteIndex::PROPERTY,
            "variable.parameter" | "variable.parameter.builtin" => {
                return PaletteIndex::PROPERTY
            }
            "variable.special" | "variable.this" => return PaletteIndex::SPECIAL,
            "markup.heading" | "markup.list" | "markup.bold" | "markup.italic" => {
                return PaletteIndex::SPECIAL
            }
            _ => {}
        }

        let base = name.split('.').next().unwrap_or(name);
        match base {
            "comment" => PaletteIndex::COMMENT,
            "keyword" => PaletteIndex::KEYWORD,
            "function" | "method" | "constructor" => PaletteIndex::FUNCTION,
            "type" => PaletteIndex::TYPE,
            "string" => PaletteIndex::STRING,
            "number" => PaletteIndex::NUMBER,
            "operator" => PaletteIndex::OPERATOR,
            "property" | "field" | "member" => PaletteIndex::PROPERTY,
            "attribute" => PaletteIndex::ATTRIBUTE,
            "tag" => PaletteIndex::TAG,
            "constant" | "symbol" | "enum" => PaletteIndex::MACRO,
            "variable" => PaletteIndex::TEXT,
            "parameter" => PaletteIndex::PROPERTY,
            "boolean" => PaletteIndex::BOOLEAN,
            "escape" | "punctuation" => PaletteIndex::SPECIAL,
            "module" | "embedded" | "label" | "namespace" | "markup" => {
                PaletteIndex::SPECIAL
            }
            _ => PaletteIndex::TEXT,
        }
    }

    fn format<Font>(&self, idx: usize) -> HighlightFormat<Font> {
        let mut format = HighlightFormat::default();
        format.color = self
            .palette
            .get(idx)
            .and_then(|color| *color)
            .or_else(|| self.palette.get(PaletteIndex::TEXT).and_then(|color| *color));
        format
    }
}

struct PaletteIndex;

impl PaletteIndex {
    const TEXT: usize = 0;
    const COMMENT: usize = 1;
    const KEYWORD: usize = 2;
    const FUNCTION: usize = 3;
    const TYPE: usize = 4;
    const STRING: usize = 5;
    const NUMBER: usize = 6;
    const OPERATOR: usize = 7;
    const PROPERTY: usize = 8;
    const MACRO: usize = 9;
    const TAG: usize = 10;
    const ATTRIBUTE: usize = 11;
    const SPECIAL: usize = 12;
    const BOOLEAN: usize = 13;
    const TOTAL: usize = 14;
}

#[derive(Default)]
struct HighlightStore {
    entries: Mutex<HashMap<DocumentKey, DocumentHighlight>>,
}

impl HighlightStore {
    fn set(&self, key: DocumentKey, highlight: DocumentHighlight) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.insert(key, highlight);
        }
    }

    fn line_spans(&self, key: &DocumentKey, line: usize) -> Vec<LineHighlight> {
        if let Ok(entries) = self.entries.lock() {
            if let Some(doc) = entries.get(key) {
                if let Some(spans) = doc.lines.get(line) {
                    return spans.clone();
                }
            }
        }

        Vec::new()
    }
}

#[derive(Clone)]
struct DocumentHighlight {
    lines: Vec<Vec<LineHighlight>>,
}

impl DocumentHighlight {
    fn with_lines(lines: Vec<Vec<LineHighlight>>) -> Self {
        Self { lines }
    }

    fn plain(text: &str) -> Self {
        let lines = line_bounds(text)
            .into_iter()
            .map(|_| Vec::new())
            .collect();
        Self { lines }
    }
}

#[derive(Clone)]
struct LineHighlight {
    range: Range<usize>,
    palette_index: usize,
}

fn highlight_document(
    text: &str,
    config: &LanguageConfig,
) -> Result<Vec<Vec<LineHighlight>>, tree_sitter_highlight::Error> {
    let mut highlighter = TsHighlighter::new();
    let mut current_style: Option<usize> = None;
    let mut stack: Vec<usize> = Vec::new();
    let bounds = line_bounds(text);
    let mut lines: Vec<Vec<LineHighlight>> = bounds.iter().map(|_| Vec::new()).collect();

    if lines.is_empty() {
        return Ok(lines);
    }
    let mut line_index = 0usize;

    for event in highlighter.highlight(&config.configuration, text.as_bytes(), None, |_| None)? {
        match event? {
            HighlightEvent::HighlightStart(id) => {
                let palette = config.highlight_id_to_palette(id.0);
                stack.push(palette);
                current_style = Some(palette);
            }
            HighlightEvent::HighlightEnd => {
                stack.pop();
                current_style = stack.last().copied();
            }
            HighlightEvent::Source { start, end } => {
                if start >= end {
                    continue;
                }

                if let Some(style) = current_style {
                    distribute_segment(
                        &mut lines,
                        &bounds,
                        &mut line_index,
                        start,
                        end,
                        style,
                    );
                }
            }
        }
    }

    Ok(lines)
}

fn distribute_segment(
    lines: &mut [Vec<LineHighlight>],
    bounds: &[LineBound],
    line_index: &mut usize,
    mut start: usize,
    end: usize,
    style: usize,
) {
    if bounds.is_empty() {
        return;
    }

    while *line_index < bounds.len() && start >= bounds[*line_index].next_start {
        *line_index += 1;
    }

    let mut current_line = *line_index;

    while current_line < bounds.len() && start < end {
        let bound = &bounds[current_line];

        let segment_start = start.max(bound.start);
        let segment_end = end.min(bound.end);

        if segment_start < segment_end {
            let range = (segment_start - bound.start)..(segment_end - bound.start);
            if !range.is_empty() {
                lines[current_line].push(LineHighlight {
                    range,
                    palette_index: style,
                });
            }
        }

        if end <= bound.end {
            break;
        }

        current_line += 1;
        start = bound.next_start;
    }

    *line_index = current_line;
}

#[derive(Clone, Copy)]
struct LineBound {
    start: usize,
    end: usize,
    next_start: usize,
}

fn line_bounds(text: &str) -> Vec<LineBound> {
    let bytes = text.as_bytes();
    let mut bounds = Vec::new();
    let mut start = 0usize;

    for (i, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            bounds.push(LineBound {
                start,
                end: i,
                next_start: i + 1,
            });
            start = i + 1;
        }
    }

    bounds.push(LineBound {
        start,
        end: text.len(),
        next_start: text.len(),
    });

    bounds
}

#[derive(Clone)]
pub struct SyntaxSettings {
    key: DocumentKey,
    store: Arc<HighlightStore>,
    theme: Arc<SyntaxTheme>,
}

impl PartialEq for SyntaxSettings {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && Arc::ptr_eq(&self.store, &other.store)
            && Arc::ptr_eq(&self.theme, &other.theme)
    }
}

#[derive(Clone)]
pub struct SyntaxHighlight {
    palette_index: usize,
    theme: Arc<SyntaxTheme>,
}

impl SyntaxHighlight {
    fn new(palette_index: usize, theme: Arc<SyntaxTheme>) -> Self {
        Self { palette_index, theme }
    }

    pub fn to_format<Font>(&self) -> HighlightFormat<Font> {
        self.theme.format(self.palette_index)
    }
}

pub fn format_highlight<Font>(highlight: &SyntaxHighlight, _theme: &iced::Theme) -> HighlightFormat<Font> {
    highlight.to_format()
}

pub struct SyntaxHighlighter {
    settings: SyntaxSettings,
    current_line: usize,
}

impl IcedHighlighter for SyntaxHighlighter {
    type Settings = SyntaxSettings;
    type Highlight = SyntaxHighlight;
    type Iterator<'a> = SyntaxIterator where Self: 'a;

    fn new(settings: &Self::Settings) -> Self {
        Self {
            settings: settings.clone(),
            current_line: 0,
        }
    }

    fn update(&mut self, new_settings: &Self::Settings) {
        self.settings = new_settings.clone();
        self.current_line = 0;
    }

    fn change_line(&mut self, line: usize) {
        self.current_line = line;
    }

    fn highlight_line(&mut self, _line: &str) -> Self::Iterator<'_> {
        let line_index = self.current_line;
        self.current_line = self.current_line.saturating_add(1);

        let spans = self
            .settings
            .store
            .line_spans(&self.settings.key, line_index)
            .into_iter();

        SyntaxIterator {
            theme: Arc::clone(&self.settings.theme),
            spans,
        }
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

pub struct SyntaxIterator {
    theme: Arc<SyntaxTheme>,
    spans: std::vec::IntoIter<LineHighlight>,
}

impl Iterator for SyntaxIterator {
    type Item = (Range<usize>, SyntaxHighlight);

    fn next(&mut self) -> Option<Self::Item> {
        self.spans.next().map(|span| {
            let highlight = SyntaxHighlight::new(span.palette_index, Arc::clone(&self.theme));
            (span.range, highlight)
        })
    }
}
