use anyhow::bail;
use clap::{Parser, Subcommand};
use marathon_api::{execute, server, task::TaskId, task::TaskRequest, worker, Client};
use std::collections::HashMap;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Clone)]
enum TaskCommand {
    Run {
        #[arg(short = 'e', long = "env")]
        environment: Vec<String>,
        cmdline: Vec<String>,
    },
    Submit {
        #[arg(long)]
        server: reqwest::Url,
        #[arg(short = 'e', long = "env")]
        environment: Vec<String>,
        cmdline: Vec<String>,
    },
    Status {
        #[arg(long)]
        server: reqwest::Url,
        id: Option<TaskId>,
    },
}

#[derive(Subcommand, Clone)]
enum Command {
    Server {
        #[arg(long, default_value = "0.0.0.0:8000")]
        listen: SocketAddr,

        /// Start a background worker thread to run tasks.
        #[arg(long)]
        start_worker: bool,
    },
    Worker {
        #[arg(long)]
        server: reqwest::Url,
    },
    #[command(subcommand)]
    Task(TaskCommand),
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

fn tracing_filter() -> EnvFilter {
    // https://github.com/tokio-rs/tracing/issues/3022
    if let Ok(filter) = std::env::var("RUST_LOG") {
        EnvFilter::new(filter)
    } else {
        EnvFilter::try_new("marathon_api=trace,info")
            .expect("hard-coded default directive should be valid")
    }
}

#[tokio::main(flavor = "current_thread")]
#[tracing::instrument(name = "server", skip_all)]
async fn start_server(addr: SocketAddr, start_worker: bool) -> anyhow::Result<()> {
    let config = server::Configuration::default();
    let server = server::ApiServer::new(config).await?;

    let listener = server.listen(addr).await?;

    // If a worker thread is requested we bind onto an additional address on a random port on
    // localhost to make sure we can reach it. Using the original listener directly isn't super
    // reliable or cross-platform (eg. when addr is `0.0.0.0`, we may not be able to use that to
    // connect).
    if start_worker {
        let extra_listener = server.listen("127.0.0.1:0").await?;
        let extra_addr = extra_listener.local_addr()?;
        let url = reqwest::Url::parse(&format!("http://{extra_addr}"))?;
        let _worker = worker::run_background(url);

        tokio::try_join!(listener, extra_listener)?;
    } else {
        listener.await?;
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_filter())
        .init();

    let args = Args::parse();
    match args.cmd {
        Command::Task(TaskCommand::Run {
            cmdline,
            environment,
        }) => {
            let environment = parse_environment(&environment, |k| std::env::var(k))?;
            let result = execute(&TaskRequest {
                cmdline,
                environment,
            })?;
            for l in result.output {
                println!("{l}");
            }
            println!("Task terminated with {}", result.status);
        }

        Command::Task(TaskCommand::Submit {
            server,
            cmdline,
            environment,
        }) => {
            let environment = parse_environment(&environment, |k| std::env::var(k))?;
            let client = Client::new(server);
            let id = client.task_submit(&TaskRequest {
                cmdline,
                environment,
            })?;
            println!("Task submitted as {id}");
        }

        Command::Task(TaskCommand::Status { server, id }) => {
            let client = Client::new(server);
            if let Some(id) = id {
                let task = client.task_get(id)?;
                println!("Task status: {:?}", task.status);
            } else {
                for task in client.task_list()? {
                    println!("{}: {:?}", task.id, task.status);
                }
            }
        }

        Command::Server {
            listen,
            start_worker,
        } => {
            start_server(listen, start_worker)?;
        }

        Command::Worker { server } => {
            worker::run(server)?;
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
