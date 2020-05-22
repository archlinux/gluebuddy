extern crate anyhow;

use args::*;
mod args;

mod keycloak;

use structopt::StructOpt;

use anyhow::Result;
use log::error;
use env_logger::Env;


async fn run(args: Args) -> Result<()> {
    match args.command {
        Command::Completions(completions) => args::gen_completions(&completions)?,
        Command::Keycloak(action) => keycloak::run(action).await?,
        Command::Plan => keycloak::run(Action::Plan).await?,
        Command::Apply => keycloak::run(Action::Apply).await?,
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::from_args();

    let logging = match args.verbose {
        0 => "info",
        1 => "gluebuddy=debug",
        _ => "debug",
    };

    env_logger::init_from_env(Env::default()
        .default_filter_or(logging));

    if let Err(err) = run(args).await {
        error!("Error: {:?}", err);
        for cause in err.chain() {
            error!("Caused by: {:?}", cause)
        }
        std::process::exit(1)
    }
}
