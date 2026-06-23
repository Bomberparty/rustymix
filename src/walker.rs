use std::path::PathBuf;
use ignore::WalkBuilder;
use crate::cli::RunArgs;

pub fn collect_files(args: &RunArgs) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
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
                if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
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
    Ok(file_paths)
}