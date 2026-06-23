use std::path::Path;
use std::fs;

pub fn init_ignore_file() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(".rustymixignore");
    if path.exists() {
        eprintln!(".rustymixignore already exists. Skipping creation.");
        return Ok(());
    }
    let content = r#"# .rustymixignore – custom ignore patterns ...
"#;
    fs::write(path, content)?;
    eprintln!("Created .rustymixignore in the current directory.");
    Ok(())
}