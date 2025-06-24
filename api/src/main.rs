use anyhow::anyhow;
use marathon_api::{task, TaskRequest};
use std::collections::HashMap;

use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Clone)]
enum Command {
    Run {
        cmdline: Vec<String>,

        #[arg(short = 'e', long = "env")]
        environment: Vec<String>,
    },
}

// Parse a list of arguments given to the -e flag, and returns a map from key to value:
// - If the argument has the form KEY=VALUE, then (KEY, VALUE) is used
// - If the argument has the form KEY and KEY exists in the current process' environment, that
//   value is used
// - If the arguments has the form KEY and KEY does not exist in the process' environment, the
//   argument is ignored
//
// This is the same behaviour used by the `docker run` command.
fn parse_environment(env: &[String]) -> anyhow::Result<HashMap<String, String>> {
    env.iter()
        .filter_map(|arg| {
            if let Some((key, value)) = arg.split_once("=") {
                Some(Ok((key.to_owned(), value.to_owned())))
            } else {
                use std::env::VarError::*;
                match std::env::var(arg) {
                    Ok(value) => Some(Ok((arg.to_owned(), value))),
                    Err(NotPresent) => None,
                    Err(NotUnicode(_)) => Some(Err(anyhow!(
                        "Environment variable {} contains invalid characters",
                        arg
                    ))),
                }
            }
        })
        .collect()
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.cmd {
        Command::Run {
            cmdline,
            environment,
        } => {
            let result = task::execute(&TaskRequest {
                cmdline,
                environment: parse_environment(&environment)?,
            })?;
            for l in result.output {
                println!("{}", l);
            }
            println!("Task terminated with {}", result.status);
        }
    }
    Ok(())
}
