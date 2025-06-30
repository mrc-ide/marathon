use anyhow::bail;
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

// Parse an argument given to the -e flag, and returns an optional key-value pair:
// - If the argument has the form KEY=VALUE, then (KEY, VALUE) is returned
// - If the argument has the form KEY and KEY exists in the current process' environment, that
//   value is used
// - If the arguments has the form KEY and KEY does not exist in the process' environment, None is
//   returned
//
// This is the same behaviour used by the `docker run` command.
fn parse_environment_arg<E>(arg: &str, getenv: E) -> anyhow::Result<Option<(String, String)>>
where
    E: Fn(&str) -> Result<String, std::env::VarError>,
{
    if let Some((key, value)) = arg.split_once("=") {
        Ok(Some((key.to_owned(), value.to_owned())))
    } else {
        use std::env::VarError::*;
        match getenv(arg) {
            Ok(value) => Ok(Some((arg.to_owned(), value))),
            Err(NotPresent) => Ok(None),
            Err(NotUnicode(_)) => bail!("Environment variable {} contains invalid characters", arg),
        }
    }
}

fn parse_environment<S, E>(args: &[S], getenv: E) -> anyhow::Result<HashMap<String, String>>
where
    S: AsRef<str>,
    E: Fn(&str) -> Result<String, std::env::VarError>,
{
    args.iter()
        .filter_map(move |arg| parse_environment_arg(arg.as_ref(), &getenv).transpose())
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
                environment: parse_environment(&environment, |k| std::env::var(k))?,
            })?;
            for l in result.output {
                println!("{}", l);
            }
            println!("Task terminated with {}", result.status);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assertor::*;

    #[test]
    fn can_parse_environment() -> anyhow::Result<()> {
        let pair = |k: &str, v: &str| (k.to_owned(), v.to_owned());

        let env: HashMap<String, String> = HashMap::from([pair("A", "foo"), pair("B", "bar")]);
        let getenv = |k: &str| env.get(k).cloned().ok_or(std::env::VarError::NotPresent);

        assert_that!(parse_environment(
            &["A", "B=override", "C", "D=value", "E=key=value"],
            getenv
        )?)
        .contains_exactly(HashMap::from([
            pair("A", "foo"),
            pair("B", "override"),
            pair("D", "value"),
            pair("E", "key=value"),
        ]));

        Ok(())
    }
}
