use languages;

pub struct CommentSyntax {
    pub single_line: &'static [&'static str],
    pub multi_line: Option<(&'static str, &'static str)>,
}

pub fn get_comment_syntax(lang_name: &str) -> Option<CommentSyntax> {
    match lang_name.to_lowercase().as_str() {
        // C-style: // and /* */
        "rust" | "c" | "c++" | "cpp" | "c#" | "cs" | "java" | "javascript" | "js" | "jsx" 
        | "typescript" | "ts" | "tsx" | "go" | "swift" | "kotlin" | "scala" | "dart" 
        | "php" | "zig" | "objective-c" | "objc" => {
            Some(CommentSyntax {
                single_line: &["//"],
                multi_line: Some(("/*", "*/")),
            })
        }
        // Hash-style: #
        "python" | "py" | "ruby" | "rb" | "perl" | "pl" | "shell" | "sh" | "bash" 
        | "yaml" | "yml" | "toml" | "dockerfile" | "powershell" | "ps1" | "r" => {
            Some(CommentSyntax {
                single_line: &["#"],
                multi_line: None,
            })
        }
        // HTML/XML: <!-- -->
        "html" | "xml" | "xhtml" | "vue" | "svelte" => {
            Some(CommentSyntax {
                single_line: &[],
                multi_line: Some(("<!--", "-->")),
            })
        }
        // CSS/SCSS: /* */ only
        "css" | "scss" | "sass" | "less" => {
            Some(CommentSyntax {
                single_line: &[],
                multi_line: Some(("/*", "*/")),
            })
        }
        // SQL: -- and /* */
        "sql" => {
            Some(CommentSyntax {
                single_line: &["--"],
                multi_line: Some(("/*", "*/")),
            })
        }
        // Lua: -- and --[[ ]]
        "lua" => {
            Some(CommentSyntax {
                single_line: &["--"],
                multi_line: Some(("--[[", "]]")),
            })
        }
        // Erlang/Elixir: # or %
        "erlang" | "erl" | "hrl" => {
            Some(CommentSyntax {
                single_line: &["%"],
                multi_line: None,
            })
        }
        "elixir" | "ex" | "exs" => {
            Some(CommentSyntax {
                single_line: &["#"],
                multi_line: None,
            })
        }
        // Fallback: no comment syntax known
        _ => None,
    }
}

pub fn analyze_content(content: &str, lang: &languages::Language) -> usize {
    let syntax = get_comment_syntax(lang.name);
    analyze_with_syntax(content, syntax.as_ref())
}

pub fn analyze_with_syntax(content: &str, syntax: Option<&CommentSyntax>) -> usize {
    let mut code = 0;
    let mut in_multi = false;
    
    let (single_comments, multi_comment) = match syntax {
        Some(s) => (s.single_line, s.multi_line),
        None => (&[][..], None),
    };

    for line in content.lines() {
        let trimmed = line.trim();
        
        if trimmed.is_empty() {
            continue;
        }

        if in_multi {
            if let Some((_, end)) = multi_comment {
                if trimmed.contains(end) {
                    in_multi = false;
                }
            }
            continue;
        }

        if single_comments.iter().any(|&prefix| trimmed.starts_with(prefix)) {
            continue;
        }

        if let Some((start, end)) = multi_comment {
            if trimmed.starts_with(start) {
                // Check if comment ends on same line (e.g., /* foo */)
                if let (Some(s_idx), Some(e_idx)) = (trimmed.find(start), trimmed.find(end)) {
                    if e_idx > s_idx && e_idx + end.len() <= trimmed.len() {
                        continue;
                    }
                }
                in_multi = true;
                continue;
            }
        }

        code += 1;
    }
    code
}