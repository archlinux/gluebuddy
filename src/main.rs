use args::*;
mod args;

use state::State;
mod state;

mod util;

mod components;
use components::gitlab::GitLabGlue;
use components::keycloak::Keycloak;

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use env_logger::Env;
use log::error;
use tokio::sync::Mutex;

async fn run(args: Args) -> Result<()> {
    /* Early exit for completions */
    match args.command {
        Command::Completions(completions) => {
            args::gen_completions(&completions)?;
            return Ok(());
        }
        _ => {}
    }

    let state = Arc::new(Mutex::new(State::default()));

    let keycloak_glue = Keycloak::new(state.clone()).await?;
    let gitlab_glue = GitLabGlue::new(state.clone()).await?;

    keycloak_glue.gather().await?;
    gitlab_glue.gather().await?;

    match args.command {
        Command::Completions(_) => {}
        Command::Keycloak { action } => {
            keycloak_glue.run(action).await?;
        }
        Command::Gitlab { action } => gitlab_glue.run(action).await?,
        Command::Plan => {
            keycloak_glue.run(Action::Plan).await?;
            gitlab_glue.run(Action::Plan).await?;
        }
        Command::Apply => {
            keycloak_glue.run(Action::Apply).await?;
            gitlab_glue.run(Action::Apply).await?;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let logging = match args.verbose {
        0 => "info",
        1 => "gluebuddy=debug",
        _ => "debug",
    };

    env_logger::init_from_env(Env::default().default_filter_or(logging));

    if let Err(err) = run(args).await {
        error!("Error: {:?}", err);
        for cause in err.chain() {
            error!("Caused by: {:?}", cause)
        }
        std::process::exit(1)
    }
}
