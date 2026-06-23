use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(flatten)]
    pub run_args: RunArgs,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Parser, Debug, Default)]
pub struct RunArgs {
    #[arg(default_value = ".")]
    pub input_dir: PathBuf,
    #[arg(short, long, default_value = "codebase.md")]
    pub output: PathBuf,
    #[arg(long)]
    pub include_hidden: bool,
    #[arg(short, long)]
    pub silent: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Init,
}