use std::collections::HashMap;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};

pub struct OutputWriter {
    writer: BufWriter<File>,
    pub language_counts: HashMap<String, usize>,
    pub total_loc: usize,
}

impl OutputWriter {
    pub async fn new(output_path: &Path) -> Result<Self, std::io::Error> {
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(output_path)
            .await?;
        Ok(Self {
            writer: BufWriter::new(file),
            language_counts: HashMap::new(),
            total_loc: 0,
        })
    }

    pub async fn write_file_entry(
        &mut self,
        rel_path: &Path,
        content: &str,
        lang_identifier: &str,
        code_lines: usize,
    ) -> Result<(), std::io::Error> {
        // Write **path**, code fence, content, and closing fence
        self.writer.write_all(b"**").await?;
        self.writer.write_all(rel_path.display().to_string().as_bytes()).await?;
        self.writer.write_all(b"**\n").await?;
        let fence = format!("```{}\n", lang_identifier);
        self.writer.write_all(fence.as_bytes()).await?;
        self.writer.write_all(content.as_bytes()).await?;
        if !content.ends_with('\n') {
            self.writer.write_all(b"\n").await?;
        }
        self.writer.write_all(b"```\n\n").await?;

        // Update statistics
        *self.language_counts.entry(lang_identifier.to_string()).or_insert(0) += code_lines;
        self.total_loc += code_lines;
        Ok(())
    }

    pub async fn flush(&mut self) -> Result<(), std::io::Error> {
        self.writer.flush().await
    }

    pub fn most_popular_language(&self) -> Option<(&String, &usize)> {
        self.language_counts.iter().max_by_key(|(_, count)| *count)
    }
}