use std::path::{Path, PathBuf};
use clap::{Parser, Subcommand};
use ignore::WalkBuilder;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::task;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Arguments for the default "run" command
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

    if verbose {
        eprintln!("Found {} files to process.", file_paths.len());
    }

    let output_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&args.output)
        .await?;
    let mut writer = BufWriter::new(output_file);

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

        write_file_entry(&mut writer, rel_path, &content).await?;
    }

    writer.flush().await?;

    if verbose {
        eprintln!("Done. Output written to {}", args.output.display());
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
