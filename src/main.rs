use args::*;
mod args;

#[allow(dead_code)]
mod state;
use state::State;

#[allow(dead_code)]
mod util;

#[allow(dead_code)]
mod components;
use components::gitlab::GitLabGlue;
use components::keycloak::Keycloak;
use components::mailman::Mailman;

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use env_logger::Env;
use log::error;
use tokio::sync::Mutex;

async fn run(args: Args) -> Result<()> {
    /* Early exit for completions */
    if let Command::Completions(completions) = args.command {
        gen_completions(&completions);
        return Ok(());
    }

    let state = Arc::new(Mutex::new(State::default()));

    let keycloak_glue = Keycloak::new(state.clone()).await?;
    let gitlab_glue = GitLabGlue::new(state.clone()).await?;
    let mailman_glue = Mailman::new(state.clone())?;

    keycloak_glue.gather().await?;
    // gitlab_glue.gather().await?;

    match args.command {
        Command::Completions(_) => {}
        Command::Keycloak { action } => {
            keycloak_glue.run(action).await?;
        }
        Command::Gitlab { action } => gitlab_glue.run(action).await?,
        Command::Plan => {
            // keycloak_glue.run(Action::Plan).await?;
            // gitlab_glue.run(Action::Plan).await?;
            mailman_glue.run(Action::Plan).await?;
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
        2 => "debug",
        _ => "trace",
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
