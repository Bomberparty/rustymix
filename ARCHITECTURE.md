# Architecture Overview
This document provides a comprehensive architectural description of **rustymix**, a command-line tool for generating a combined Markdown file of a codebase, with per-file syntax highlighting and basic code statistics.

## 1. Project Structure
The project is a single Rust binary with a straightforward structure:

```
rustymix/
├── Cargo.toml           # Project manifest and dependencies
├── .rustymixignore      # Optional custom ignore file (created via `init` command)
└── src/
    └── main.rs          # Entire application source code
```

All logic resides in one file (`main.rs`), organised into:
- **CLI argument parsing** (`clap` derive macros)
- **Ignore file handling** (`ignore` crate for `.gitignore` + custom ignore)
- **File traversal and collection**
- **Language detection** (`languages` crate)
- **Comment syntax definitions** (manual mapping for many languages)
- **Line‑of‑code counting** (excluding blanks and comments)
- **Markdown output generation** (with fenced code blocks)
- **Asynchronous I/O** (`tokio` for writing output)

No separate backend/frontend or common modules exist – the tool is self‑contained.

## 2. High-Level System Diagram
```
[User] --> (CLI command with arguments)
               |
               v
        [rustymix]
               |
               +--> Reads file system (input directory)
               +--> Applies .gitignore + .rustymixignore filters
               +--> For each file:
               |       - detect language (by extension)
               |       - count code lines (ignoring blanks/comments)
               |       - write file path & code block to output
               +--> Writes summary statistics to stdout
               |
               v
        [Output Markdown file]
```

The tool is entirely offline and does not interact with external services.

## 3. Core Components

### 3.1. CLI & Argument Parsing
- **Component**: `Cli`, `RunArgs`, `Commands`
- **Description**: Defines command‑line arguments and subcommands (`init`) using `clap`. The `RunArgs` specify input directory, output file, hidden file inclusion, and silent mode.
- **Technology**: `clap` (derive feature)

### 3.2. File Walker
- **Component**: `ignore::WalkBuilder`
- **Description**: Recursively traverses the input directory, respecting `.gitignore` and the custom `.rustymixignore` file. Filters out hidden files unless explicitly requested.
- **Technology**: `ignore` crate

### 3.3. Language Detection & Comment Analysis
- **Component**: `get_comment_syntax`, `analyze_content`
- **Description**: Maps file extensions to languages (via `languages` crate) and then to a set of single‑line and multi‑line comment syntaxes. Counts lines of actual code (excluding blanks and comments) per file.
- **Technology**: Manual mapping (extensible), `languages` crate for language names.

### 3.4. Output Writer
- **Component**: `write_file_entry`, `run_dump` (async)
- **Description**: Asynchronously writes each file’s relative path and its content inside a fenced code block (with language identifier) to the output Markdown file. Aggregates total lines of code and tracks the most frequent language.
- **Technology**: `tokio` for async file I/O, `BufWriter` for performance.

### 3.5. Initialization Command
- **Component**: `init_ignore_file`
- **Description**: Creates a default `.rustymixignore` file in the current directory with sensible exclusions (build artifacts, IDE files, output file itself).
- **Technology**: Standard `std::fs`.

## 4. Data Stores
The application does not use any external database or persistent storage beyond the file system.

- **Input**: Reads files from the user‑specified directory.
- **Output**: Writes a single Markdown file (default `codebase.md`).
- **Configuration**: Optional `.rustymixignore` file (plain text).

## 5. External Integrations / APIs
None – the tool is fully self‑contained and does not call any external services or APIs.

## 6. Deployment & Infrastructure
- **Build**: Compiled to a native binary using `cargo build --release`.
- **Distribution**: Can be installed via `cargo install` or by downloading the binary.
- **No cloud dependencies**: Runs entirely on the user’s local machine.

## 7. Security Considerations
- The tool only reads files and writes one output file; it does not execute or modify any code.
- No network communication occurs, so there is no exposure to remote attacks.
- Users should review `.rustymixignore` contents to avoid inadvertently including sensitive files.
- The tool respects `.gitignore` by default, reducing the risk of dumping ignored files.

## 8. Development & Testing Environment
- **Rust Toolchain**: Edition 2024, requires Rust 1.70+.
- **Dependencies**:
  - `clap` for CLI parsing
  - `ignore` for walking and ignore rules
  - `languages` for language detection
  - `tokio` for async runtime
- **Testing**: Currently no automated tests; manual testing recommended.
- **Code Quality**: Uses standard Rust formatting (`cargo fmt`) and linting (`cargo clippy`).

## 9. Future Considerations / Roadmap
- Improve code quality and modularize the main codebase
- Add option to exclude specific file patterns via command line.
- Generate a table of contents or summary section in the output file.
- Generate output files in several formats (XML, JSON, Markdown)

## 10. Project Identification
- **Project Name**: rustymix
- **Repository URL**: (not specified – source can be found at the provided Cargo.toml location)
- **Primary Contact**: (open source – maintainer unknown)
- **Date of Last Update**: 2026-06-21 (based on current context)

## 11. Glossary / Acronyms
- **LOC**: Lines of Code (excluding blank lines and comments).
- **Markdown**: Lightweight markup language used for the output.
- **.rustymixignore**: Custom ignore file that works alongside `.gitignore`.
