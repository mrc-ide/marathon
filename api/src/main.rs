use marathon_api::{task, TaskRequest};

use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Clone)]
enum Command {
    Run { cmdline: Vec<String> },
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.cmd {
        Command::Run { cmdline } => {
            task::execute(&TaskRequest { cmdline })?;
        }
    }

    Ok(())
}
