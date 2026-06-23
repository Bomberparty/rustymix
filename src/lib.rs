pub mod cli;
pub mod init;
pub mod analysis;
pub mod walker;
pub mod output;

use crate::cli::RunArgs;
use crate::walker::collect_files;
use crate::output::OutputWriter;
use crate::analysis::analyze_content;
use languages;

pub async fn run(args: RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    let file_paths = collect_files(&args)?;

    let mut writer = OutputWriter::new(&args.output).await?;

    for rel_path in &file_paths {
        let absolute_path = args.input_dir.join(rel_path);
        if !args.silent {
            eprintln!("Reading: {}", rel_path.display());
        }

        let content = tokio::task::spawn_blocking({
            let path = absolute_path.clone();
            move || std::fs::read_to_string(&path)
        })
        .await??;

        let ext = rel_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let (code_lines, lang_id) = if let Some(lang) = languages::from_extension(&ext) {
            let lines = analyze_content(&content, lang);
            (lines, lang.name.to_lowercase())
        } else {
            let lines = content.lines().filter(|l| !l.trim().is_empty()).count();
            (lines, ext.clone()) // use extension as fallback
        };

        writer.write_file_entry(rel_path, &content, &lang_id, code_lines).await?;
    }

    writer.flush().await?;

    // Print summary
    println!("Done. Output written to {}", args.output.display());
    println!("lines of code: {}", writer.total_loc);
    if let Some((lang, _)) = writer.most_popular_language() {
        println!("most popular language in the codebase: {}", lang);
    }

    Ok(())
}
