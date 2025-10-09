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
    fn language_display_names() {
        let test_cases = vec![
            (Language::PlainText, "Plain Text"),
            (Language::Rust, "Rust"),
            (Language::C, "C"),
            (Language::CHeader, "C Header"),
            (Language::Cpp, "C++"),
            (Language::CppHeader, "C++ Header"),
            (Language::ObjectiveC, "Objective-C"),
            (Language::ObjectiveCpp, "Objective-C++"),
            (Language::Swift, "Swift"),
            (Language::Java, "Java"),
            (Language::Kotlin, "Kotlin"),
            (Language::CSharp, "C#"),
            (Language::Go, "Go"),
            (Language::Python, "Python"),
            (Language::Ruby, "Ruby"),
            (Language::Php, "PHP"),
            (Language::Haskell, "Haskell"),
            (Language::Erlang, "Erlang"),
            (Language::Elixir, "Elixir"),
            (Language::JavaScript, "JavaScript"),
            (Language::Jsx, "JavaScript JSX"),
            (Language::TypeScript, "TypeScript"),
            (Language::Tsx, "TypeScript JSX"),
            (Language::Json, "JSON"),
            (Language::Toml, "TOML"),
            (Language::Yaml, "YAML"),
            (Language::Ini, "INI"),
            (Language::Markdown, "Markdown"),
            (Language::Sql, "SQL"),
            (Language::Html, "HTML"),
            (Language::Css, "CSS"),
            (Language::Scss, "SCSS"),
            (Language::Less, "Less"),
            (Language::Lua, "Lua"),
            (Language::Zig, "Zig"),
            (Language::Dart, "Dart"),
            (Language::Scala, "Scala"),
            (Language::Shell, "Shell"),
            (Language::Fish, "Fish"),
            (Language::PowerShell, "PowerShell"),
            (Language::Batch, "Batch"),
            (Language::Vue, "Vue"),
            (Language::Svelte, "Svelte"),
            (Language::Makefile, "Makefile"),
            (Language::Dockerfile, "Dockerfile"),
            (Language::CMake, "CMake"),
            (Language::Nix, "Nix"),
        ];

        for (language, expected_name) in test_cases {
            assert_eq!(language.display_name(), expected_name);
        }
    }

    #[test]
    fn language_display_formatting() {
        assert_eq!(format!("{}", Language::Rust), "Rust");
        assert_eq!(format!("{}", Language::JavaScript), "JavaScript");
        assert_eq!(format!("{}", Language::PlainText), "Plain Text");
        assert_eq!(format!("{}", Language::CSharp), "C#");
        assert_eq!(format!("{}", Language::TypeScript), "TypeScript");
    }

    #[test]
    fn language_equality() {
        assert_eq!(Language::Rust, Language::Rust);
        assert_ne!(Language::Rust, Language::Python);
        assert_eq!(Language::JavaScript, Language::JavaScript);
        assert_ne!(Language::JavaScript, Language::Jsx);
    }

    #[test]
    fn language_hashing() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(Language::Rust);
        set.insert(Language::Python);
        set.insert(Language::JavaScript);

        assert!(set.contains(&Language::Rust));
        assert!(set.contains(&Language::Python));
        assert!(set.contains(&Language::JavaScript));
        assert!(!set.contains(&Language::TypeScript));

        // Duplicate insertions should not change the set
        set.insert(Language::Rust);
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn language_debug_formatting() {
        let debug_str = format!("{:?}", Language::Rust);
        assert!(debug_str.contains("Rust"));

        let debug_str = format!("{:?}", Language::JavaScript);
        assert!(debug_str.contains("JavaScript"));
    }

    #[test]
    fn language_copy_and_clone() {
        let lang1 = Language::Rust;
        let lang2 = lang1;
        assert_eq!(lang1, lang2);

        let lang3 = Language::Python;
        let lang4 = lang3.clone();
        assert_eq!(lang3, lang4);
    }

    #[test]
    fn language_c_like_variants() {
        let c_languages = vec![
            Language::C,
            Language::CHeader,
            Language::Cpp,
            Language::CppHeader,
            Language::ObjectiveC,
            Language::ObjectiveCpp,
        ];

        for lang in &c_languages {
            let display_name = lang.display_name();
            assert!(!display_name.is_empty());
            assert!(display_name.contains("C") || display_name.contains("Objective"));
        }
    }

    #[test]
    fn language_web_variants() {
        let web_languages = vec![
            Language::JavaScript,
            Language::Jsx,
            Language::TypeScript,
            Language::Tsx,
            Language::Html,
            Language::Css,
            Language::Scss,
            Language::Less,
            Language::Vue,
            Language::Svelte,
            Language::Json,
        ];

        for lang in &web_languages {
            let display_name = lang.display_name();
            assert!(!display_name.is_empty());
        }

        // Check specific naming patterns
        assert_eq!(Language::Jsx.display_name(), "JavaScript JSX");
        assert_eq!(Language::Tsx.display_name(), "TypeScript JSX");
        assert_eq!(Language::Scss.display_name(), "SCSS");
        assert_eq!(Language::Less.display_name(), "Less");
    }

    #[test]
    fn language_shell_variants() {
        let shell_languages = vec![
            Language::Shell,
            Language::Fish,
            Language::PowerShell,
            Language::Batch,
        ];

        for lang in &shell_languages {
            let display_name = lang.display_name();
            assert!(!display_name.is_empty());
        }

        assert_eq!(Language::Shell.display_name(), "Shell");
        assert_eq!(Language::Fish.display_name(), "Fish");
        assert_eq!(Language::PowerShell.display_name(), "PowerShell");
        assert_eq!(Language::Batch.display_name(), "Batch");
    }

    #[test]
    fn language_build_tools() {
        let build_tools = vec![
            Language::Makefile,
            Language::Dockerfile,
            Language::CMake,
            Language::Nix,
        ];

        for lang in &build_tools {
            let display_name = lang.display_name();
            assert!(!display_name.is_empty());
        }

        assert_eq!(Language::Makefile.display_name(), "Makefile");
        assert_eq!(Language::Dockerfile.display_name(), "Dockerfile");
        assert_eq!(Language::CMake.display_name(), "CMake");
        assert_eq!(Language::Nix.display_name(), "Nix");
    }

    #[test]
    fn language_config_formats() {
        let config_formats = vec![
            Language::Json,
            Language::Toml,
            Language::Yaml,
            Language::Ini,
        ];

        for lang in &config_formats {
            let display_name = lang.display_name();
            assert!(!display_name.is_empty());
            // All config formats should be uppercase
            assert_eq!(display_name, display_name.to_uppercase());
        }
    }

    #[test]
    fn language_all_unique() {
        let mut seen = std::collections::HashSet::new();
        let all_languages = vec![
            Language::PlainText,
            Language::Rust,
            Language::C,
            Language::CHeader,
            Language::Cpp,
            Language::CppHeader,
            Language::ObjectiveC,
            Language::ObjectiveCpp,
            Language::Swift,
            Language::Java,
            Language::Kotlin,
            Language::CSharp,
            Language::Go,
            Language::Python,
            Language::Ruby,
            Language::Php,
            Language::Haskell,
            Language::Erlang,
            Language::Elixir,
            Language::JavaScript,
            Language::Jsx,
            Language::TypeScript,
            Language::Tsx,
            Language::Json,
            Language::Toml,
            Language::Yaml,
            Language::Ini,
            Language::Markdown,
            Language::Sql,
            Language::Html,
            Language::Css,
            Language::Scss,
            Language::Less,
            Language::Lua,
            Language::Zig,
            Language::Dart,
            Language::Scala,
            Language::Shell,
            Language::Fish,
            Language::PowerShell,
            Language::Batch,
            Language::Vue,
            Language::Svelte,
            Language::Makefile,
            Language::Dockerfile,
            Language::CMake,
            Language::Nix,
        ];

        for lang in all_languages {
            assert!(!seen.contains(&lang), "Duplicate language found: {:?}", lang);
            seen.insert(lang);
        }

        // Should have exactly the expected number of unique languages
        assert_eq!(seen.len(), 47);
    }

    #[test]
    fn language_comprehensive_coverage() {
        // Test that we have coverage for major language categories
        let categories = vec![
            // Systems programming
            (Language::Rust, "Systems"),
            (Language::C, "Systems"),
            (Language::Cpp, "Systems"),
            (Language::Zig, "Systems"),

            // Web development
            (Language::JavaScript, "Web"),
            (Language::TypeScript, "Web"),
            (Language::Html, "Web"),
            (Language::Css, "Web"),

            // Mobile/Modern
            (Language::Swift, "Mobile"),
            (Language::Kotlin, "Mobile"),
            (Language::Dart, "Mobile"),

            // Data/Scientific
            (Language::Python, "Data"),
            (Language::Shell, "Data"), // Using Shell as placeholder for data scripting

            // Enterprise
            (Language::Java, "Enterprise"),
            (Language::CSharp, "Enterprise"),
            (Language::Scala, "Enterprise"),

            // Scripting
            (Language::Shell, "Scripting"),
            (Language::Ruby, "Scripting"),
            (Language::Php, "Scripting"),

            // Functional
            (Language::Haskell, "Functional"),
            (Language::Erlang, "Functional"),
            (Language::Elixir, "Functional"),
        ];

        for (lang, _category) in categories {
            let display_name = lang.display_name();
            assert!(!display_name.is_empty());
            assert!(display_name.len() >= 1); // Should have meaningful names
        }
    }

    #[test]
    fn language_special_characters() {
        // Test languages with special characters in their display names
        assert_eq!(Language::CSharp.display_name(), "C#");
        assert_eq!(Language::Cpp.display_name(), "C++");
        assert_eq!(Language::ObjectiveC.display_name(), "Objective-C");
        assert_eq!(Language::ObjectiveCpp.display_name(), "Objective-C++");

        // Ensure these format correctly in Display trait
        assert_eq!(format!("{}", Language::CSharp), "C#");
        assert_eq!(format!("{}", Language::Cpp), "C++");
        assert_eq!(format!("{}", Language::ObjectiveC), "Objective-C");
        assert_eq!(format!("{}", Language::ObjectiveCpp), "Objective-C++");
    }

    #[test]
    fn language_case_consistency() {
        // Test that display names follow consistent patterns
        let all_languages = vec![
            Language::PlainText,
            Language::Rust,
            Language::C,
            Language::CHeader,
            Language::Cpp,
            Language::CppHeader,
            Language::ObjectiveC,
            Language::ObjectiveCpp,
            Language::Swift,
            Language::Java,
            Language::Kotlin,
            Language::CSharp,
            Language::Go,
            Language::Python,
            Language::Ruby,
            Language::Php,
            Language::Haskell,
            Language::Erlang,
            Language::Elixir,
            Language::JavaScript,
            Language::Jsx,
            Language::TypeScript,
            Language::Tsx,
            Language::Json,
            Language::Toml,
            Language::Yaml,
            Language::Ini,
            Language::Markdown,
            Language::Sql,
            Language::Html,
            Language::Css,
            Language::Scss,
            Language::Less,
            Language::Lua,
            Language::Zig,
            Language::Dart,
            Language::Scala,
            Language::Shell,
            Language::Fish,
            Language::PowerShell,
            Language::Batch,
            Language::Vue,
            Language::Svelte,
            Language::Makefile,
            Language::Dockerfile,
            Language::CMake,
            Language::Nix,
        ];

        for lang in all_languages {
            let display_name = lang.display_name();

            // No display names should be empty
            assert!(!display_name.is_empty(), "Display name should not be empty for {:?}", lang);

            // No display names should be all lowercase (except for specific cases)
            if !matches!(lang, Language::PlainText | Language::Makefile | Language::Dockerfile | Language::CMake | Language::Nix | Language::Json | Language::Toml | Language::Yaml | Language::Ini | Language::Sql | Language::Html | Language::Css | Language::Scss | Language::Less | Language::Lua | Language::Zig | Language::Dart | Language::Scala | Language::Vue | Language::Svelte) {
                // Most programming languages should have proper capitalization
                assert!(display_name.chars().next().unwrap().is_uppercase() ||
                       display_name.contains("C#") ||
                       display_name.contains("C++") ||
                       display_name.starts_with("Objective-"),
                       "Display name should be properly capitalized: {}", display_name);
            }
        }
    }

    #[test]
    fn language_extensibility_compatibility() {
        // This test ensures that the Language enum is structured in a way
        // that would be easy to extend with new languages in the future

        let current_count = 47; // Update this when adding new languages

        let all_languages = vec![
            Language::PlainText,
            Language::Rust,
            Language::C,
            Language::CHeader,
            Language::Cpp,
            Language::CppHeader,
            Language::ObjectiveC,
            Language::ObjectiveCpp,
            Language::Swift,
            Language::Java,
            Language::Kotlin,
            Language::CSharp,
            Language::Go,
            Language::Python,
            Language::Ruby,
            Language::Php,
            Language::Haskell,
            Language::Erlang,
            Language::Elixir,
            Language::JavaScript,
            Language::Jsx,
            Language::TypeScript,
            Language::Tsx,
            Language::Json,
            Language::Toml,
            Language::Yaml,
            Language::Ini,
            Language::Markdown,
            Language::Sql,
            Language::Html,
            Language::Css,
            Language::Scss,
            Language::Less,
            Language::Lua,
            Language::Zig,
            Language::Dart,
            Language::Scala,
            Language::Shell,
            Language::Fish,
            Language::PowerShell,
            Language::Batch,
            Language::Vue,
            Language::Svelte,
            Language::Makefile,
            Language::Dockerfile,
            Language::CMake,
            Language::Nix,
        ];

        assert_eq!(all_languages.len(), current_count, "Language count has changed - update this test");

        // All languages should have display names
        for lang in &all_languages {
            assert!(!lang.display_name().is_empty());
        }

        // All languages should be hashable and comparable
        for lang in &all_languages {
            let _hash = std::hash::Hash::hash(lang, &mut std::collections::hash_map::DefaultHasher::new());
            let _debug = format!("{:?}", lang);
            let _display = format!("{}", lang);
        }
    }
}