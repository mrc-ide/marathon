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
            let result = task::execute(&TaskRequest { cmdline })?;
            for l in result.output {
                println!("{}", l);
            }
            println!("Task terminated with {}", result.status);
        }
    }
    Ok(())
}
