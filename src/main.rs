use std::collections::HashMap;
use std::path::{Path, PathBuf};
use clap::{Parser, Subcommand};
use ignore::WalkBuilder;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::task;
use languages;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(flatten)]
    run_args: RunArgs,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Parser, Debug, Default)]
struct RunArgs {
    /// Root directory to scan (defaults to current directory)
    #[arg(default_value = ".")]
    input_dir: PathBuf,

    /// Output file where the combined codebase will be written
    #[arg(short, long, default_value = "codebase.md")]
    output: PathBuf,

    /// Also include hidden files/directories (starting with '.') that are not ignored
    #[arg(long)]
    include_hidden: bool,

    /// Suppress progress information (quiet mode)
    #[arg(short, long)]
    silent: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Create a default .rustymixignore file in the current directory
    Init,
}

fn init_ignore_file() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(".rustymixignore");
    if path.exists() {
        eprintln!(".rustymixignore already exists. Skipping creation.");
        return Ok(());
    }

    let content = r#"# .rustymixignore – custom ignore patterns for the codebase dumper
# This file is combined with .gitignore when scanning.
# Patterns are relative to the directory containing this file.

# Typical build output
/target/
**/target/

# IDE / editor files
.idea/
.vscode/
*.swp

# The output file itself (avoid recursive dumping)
codebase.md
*.md

# OS metadata
.DS_Store
Thumbs.db
"#;

    std::fs::write(path, content)?;
    eprintln!("Created .rustymixignore in the current directory.");
    Ok(())
}

// ──────────────────────────────────────────────────────────────
// Comment syntax definitions for line counting
// ──────────────────────────────────────────────────────────────

struct CommentSyntax {
    single_line: &'static [&'static str],
    multi_line: Option<(&'static str, &'static str)>,
}

fn get_comment_syntax(lang_name: &str) -> Option<CommentSyntax> {
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

/// Counts actual lines of code, ignoring blanks and comments.
/// Uses a manual mapping of language names to comment syntax.
fn analyze_content(content: &str, lang: &languages::Language) -> usize {
    let syntax = get_comment_syntax(lang.name);
    analyze_with_syntax(content, syntax.as_ref())
}

fn analyze_with_syntax(content: &str, syntax: Option<&CommentSyntax>) -> usize {
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

// ──────────────────────────────────────────────────────────────
// Main & Execution
// ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init) => {
            init_ignore_file()?;
        }
        None => {
            run_dump(cli.run_args).await?;
        }
    }
    Ok(())
}

async fn run_dump(args: RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mut walk_builder = WalkBuilder::new(&args.input_dir);
    walk_builder
        .git_ignore(true)
        .add_custom_ignore_filename(".rustymixignore")
        .hidden(!args.include_hidden)
        .follow_links(false);

    let walker = walk_builder.build();

    let mut file_paths = Vec::new();
    for entry in walker {
        match entry {
            Ok(entry) => {
                let metadata = entry.metadata()?;
                if metadata.is_file() {
                    let relative = entry
                        .path()
                        .strip_prefix(&args.input_dir)
                        .unwrap_or(entry.path())
                        .to_path_buf();
                    file_paths.push(relative);
                }
            }
            Err(err) => {
                if !args.silent {
                    eprintln!("Warning: skipping entry due to error: {}", err);
                }
            }
        }
    }

    let output_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&args.output)
        .await?;
    let mut writer = BufWriter::new(output_file);

    let mut language_counts: HashMap<String, usize> = HashMap::new();
    let mut total_loc = 0;

    for rel_path in &file_paths {
        let absolute_path = args.input_dir.join(rel_path);
        if !args.silent {
            eprintln!("Reading: {}", rel_path.display());
        }

        let content = task::spawn_blocking({
            let path = absolute_path.clone();
            move || std::fs::read_to_string(&path)
        })
        .await??;

        let ext = rel_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let loc_for_file = if let Some(lang) = languages::from_extension(&ext) {
            let code_lines = analyze_content(&content, lang);
            *language_counts.entry(lang.name.to_string()).or_insert(0) += code_lines;
            
            // Use lowercase language name for markdown syntax highlighting compatibility
            let lang_id = lang.name.to_lowercase();
            write_file_entry(&mut writer, rel_path, &content, &lang_id).await?;
            code_lines
        } else {
            // Fallback for unknown extensions: count non-blank lines
            let code_lines = content.lines().filter(|l| !l.trim().is_empty()).count();
            *language_counts.entry("Other".to_string()).or_insert(0) += code_lines;
            
            // Use the raw extension as fallback identifier
            write_file_entry(&mut writer, rel_path, &content, &ext).await?;
            code_lines
        };

        total_loc += loc_for_file;
    }

    writer.flush().await?;

    // ──────────────────────────────────────────────────────────────
    // Summary output (printed at the end)
    // ──────────────────────────────────────────────────────────────
    println!("Done. Output written to {}", args.output.display());
    println!("lines of code: {}", total_loc);
    if let Some((lang, _)) = language_counts.iter().max_by_key(|&(_, count)| count) {
        println!("most popular language in the codebase: {}", lang);
    }

    Ok(())
}

async fn write_file_entry(
    writer: &mut BufWriter<File>,
    relative_path: &Path,
    content: &str,
    lang_identifier: &str,
) -> Result<(), std::io::Error> {
    writer.write_all(b"**").await?;
    writer.write_all(relative_path.display().to_string().as_bytes()).await?;
    writer.write_all(b"**\n").await?;

    let fence = format!("```{}\n", lang_identifier);
    writer.write_all(fence.as_bytes()).await?;

    writer.write_all(content.as_bytes()).await?;

    if !content.ends_with('\n') {
        writer.write_all(b"\n").await?;
    }
    writer.write_all(b"```\n\n").await?;
    Ok(())
}

