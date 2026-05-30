use std::collections::HashMap;
use std::path::{Path, PathBuf};
use clap::{Parser, Subcommand};
use ignore::WalkBuilder;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::task;

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
// Language configuration & line-counting logic (adapted from cloc-rs)
// ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct LangConfig {
    name: &'static str,
    single: &'static [&'static str],
    multi: &'static [(&'static str, &'static str)],
}

fn get_lang_config(ext: &str) -> Option<LangConfig> {
    match ext {
        "rs" => Some(LangConfig { name: "Rust", single: &["//", "///", "//!"], multi: &[("/*", "*/")] }),
        "c" | "cpp" | "cxx" | "cc" | "h" | "hpp" => Some(LangConfig { name: "C/C++", single: &["//"], multi: &[("/*", "*/")] }),
        "py" => Some(LangConfig { name: "Python", single: &["#"], multi: &[("'''", "'''"), ("\"\"\"", "\"\"\"")] }),
        "js" | "mjs" | "jsx" => Some(LangConfig { name: "JavaScript", single: &["//"], multi: &[("/*", "*/")] }),
        "ts" | "tsx" => Some(LangConfig { name: "TypeScript", single: &["//"], multi: &[("/*", "*/")] }),
        "go" => Some(LangConfig { name: "Go", single: &["//"], multi: &[("/*", "*/")] }),
        "java" => Some(LangConfig { name: "Java", single: &["//"], multi: &[("/*", "*/")] }),
        "sh" | "bash" | "zsh" | "fish" => Some(LangConfig { name: "Shell", single: &["#"], multi: &[] }),
        "md" | "markdown" => Some(LangConfig { name: "Markdown", single: &[], multi: &[] }),
        "json" => Some(LangConfig { name: "JSON", single: &[], multi: &[] }),
        "yaml" | "yml" | "toml" | "ini" | "cfg" => Some(LangConfig { name: "Config", single: &["#"], multi: &[] }),
        "html" | "xml" | "svg" => Some(LangConfig { name: "Markup", single: &[], multi: &[("<!--", "-->")] }),
        "css" | "less" | "scss" | "sass" => Some(LangConfig { name: "CSS", single: &["//"], multi: &[("/*", "*/")] }),
        "lua" => Some(LangConfig { name: "Lua", single: &["--"], multi: &[("--[[", "]]")] }),
        "rb" | "ruby" => Some(LangConfig { name: "Ruby", single: &["#"], multi: &[("=begin", "=end")] }),
        "php" => Some(LangConfig { name: "PHP", single: &["#", "//"], multi: &[("/*", "*/")] }),
        "swift" => Some(LangConfig { name: "Swift", single: &["//"], multi: &[("/*", "*/")] }),
        "kt" | "kts" => Some(LangConfig { name: "Kotlin", single: &["//"], multi: &[("/*", "*/")] }),
        _ => None,
    }
}

/// Counts actual lines of code, ignoring blanks and comments.
/// Fixed lifetime issue: Option now owns the &'static str references.
fn analyze_content(content: &str, config: &LangConfig) -> usize {
    let mut code = 0;
    // FIX: Changed from Option<&'static ...> to Option<(&'static str, &'static str)>
    let mut in_multi: Option<(&'static str, &'static str)> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Inside multi-line comment
        // FIX: Prefixed unused `start` with underscore to silence warning
        if let Some((_start, end)) = in_multi {
            if line.ends_with(end) {
                in_multi = None;
            }
            continue;
        }

        // Single-line comments
        if config.single.iter().any(|&s| line.starts_with(s)) {
            continue;
        }

        // Check for multi-line comment start
        let mut found_start = None;
        for &(start, end) in config.multi {
            if line.starts_with(start) {
                found_start = Some((start, end));
                break;
            }
        }

        if let Some((start, end)) = found_start {
            // If it starts and ends on the same line (e.g., `/* comment */`)
            if !(line.ends_with(end) && line.len() >= start.len() + end.len()) {
                // FIX: Store owned tuple instead of borrowing a temporary
                in_multi = Some((start, end));
            }
            continue;
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
    let verbose = !args.silent;

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
                if verbose {
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
        if verbose {
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

        let loc_for_file = if let Some(config) = get_lang_config(&ext) {
            let code_lines = analyze_content(&content, &config);
            *language_counts.entry(config.name.to_string()).or_insert(0) += code_lines;
            code_lines
        } else {
            // Fallback for unknown extensions: count non-blank lines
            let code_lines = content.lines().filter(|l| !l.trim().is_empty()).count();
            *language_counts.entry("Other".to_string()).or_insert(0) += code_lines;
            code_lines
        };

        total_loc += loc_for_file;
        write_file_entry(&mut writer, rel_path, &content).await?;
    }

    writer.flush().await?;

    if verbose {
        eprintln!("Done. Output written to {}", args.output.display());
    }

    // ──────────────────────────────────────────────────────────────
    // Summary output (printed at the end)
    // ──────────────────────────────────────────────────────────────
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
) -> Result<(), std::io::Error> {
    writer.write_all(b"**").await?;
    writer.write_all(relative_path.display().to_string().as_bytes()).await?;
    writer.write_all(b"**\n").await?;

    let extension = relative_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    let fence = format!("```{}\n", extension);
    writer.write_all(fence.as_bytes()).await?;

    writer.write_all(content.as_bytes()).await?;

    if !content.ends_with('\n') {
        writer.write_all(b"\n").await?;
    }
    writer.write_all(b"```\n\n").await?;
    Ok(())
}

