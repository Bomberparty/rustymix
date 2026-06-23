use rustymix::{cli::{Cli, Commands}, init::init_ignore_file, run};
use clap::Parser;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init) => {
            init_ignore_file()?;
        }
        None => {
            run(cli.run_args).await?;
        }
    }
    Ok(())
}