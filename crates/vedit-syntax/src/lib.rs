use std::fmt;

/// Programming languages the editor can recognize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    PlainText,
    Rust,
    C,
    CHeader,
    Cpp,
    CppHeader,
    ObjectiveC,
    ObjectiveCpp,
    Swift,
    Java,
    Kotlin,
    CSharp,
    Go,
    Python,
    Ruby,
    Php,
    Haskell,
    Erlang,
    Elixir,
    JavaScript,
    Jsx,
    TypeScript,
    Tsx,
    Json,
    Toml,
    Yaml,
    Ini,
    Markdown,
    Sql,
    Html,
    Css,
    Scss,
    Less,
    Lua,
    Zig,
    Dart,
    Scala,
    Shell,
    Fish,
    PowerShell,
    Batch,
    Vue,
    Svelte,
    Makefile,
    Dockerfile,
    CMake,
    Nix,
}

impl Language {
    /// Human friendly label.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::PlainText => "Plain Text",
            Self::Rust => "Rust",
            Self::C => "C",
            Self::CHeader => "C Header",
            Self::Cpp => "C++",
            Self::CppHeader => "C++ Header",
            Self::ObjectiveC => "Objective-C",
            Self::ObjectiveCpp => "Objective-C++",
            Self::Swift => "Swift",
            Self::Java => "Java",
            Self::Kotlin => "Kotlin",
            Self::CSharp => "C#",
            Self::Go => "Go",
            Self::Python => "Python",
            Self::Ruby => "Ruby",
            Self::Php => "PHP",
            Self::Haskell => "Haskell",
            Self::Erlang => "Erlang",
            Self::Elixir => "Elixir",
            Self::JavaScript => "JavaScript",
            Self::Jsx => "JavaScript JSX",
            Self::TypeScript => "TypeScript",
            Self::Tsx => "TypeScript JSX",
            Self::Json => "JSON",
            Self::Toml => "TOML",
            Self::Yaml => "YAML",
            Self::Ini => "INI",
            Self::Markdown => "Markdown",
            Self::Sql => "SQL",
            Self::Html => "HTML",
            Self::Css => "CSS",
            Self::Scss => "SCSS",
            Self::Less => "Less",
            Self::Lua => "Lua",
            Self::Zig => "Zig",
            Self::Dart => "Dart",
            Self::Scala => "Scala",
            Self::Shell => "Shell",
            Self::Fish => "Fish",
            Self::PowerShell => "PowerShell",
            Self::Batch => "Batch",
            Self::Vue => "Vue",
            Self::Svelte => "Svelte",
            Self::Makefile => "Makefile",
            Self::Dockerfile => "Dockerfile",
            Self::CMake => "CMake",
            Self::Nix => "Nix",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_special_characters() {
        // Test that languages with special characters in their display names are handled correctly
        assert_eq!(Language::CSharp.display_name(), "C#");
        assert_eq!(Language::Cpp.display_name(), "C++");
        assert_eq!(Language::ObjectiveC.display_name(), "Objective-C");
        assert_eq!(Language::ObjectiveCpp.display_name(), "Objective-C++");
    }

    #[test]
    fn language_jsx_tsx_naming() {
        // Test that JSX/TSX variants have proper naming that includes the base language
        assert_eq!(Language::Jsx.display_name(), "JavaScript JSX");
        assert_eq!(Language::Tsx.display_name(), "TypeScript JSX");
    }

    #[test]
    fn language_all_unique() {
        use std::collections::HashSet;

        let languages = [
            Language::Rust,
            Language::Python,
            Language::JavaScript,
            Language::TypeScript,
            Language::CSharp,
            Language::Java,
            Language::Go,
            Language::Kotlin,
            Language::Swift,
        ];

        let mut set = HashSet::new();
        for lang in languages {
            assert!(set.insert(lang), "Language should be unique: {:?}", lang);
        }
        assert_eq!(set.len(), languages.len());
    }
}
