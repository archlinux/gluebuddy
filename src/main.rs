extern crate anyhow;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate strum;
#[macro_use]
extern crate strum_macros;

use args::*;
mod args;

use state::State;
mod state;

mod mods;
use mods::gitlab::GitLabGlue;
use mods::keycloak::Keycloak;

use structopt::StructOpt;

use anyhow::Result;
use env_logger::Env;
use log::error;

async fn run(args: Args) -> Result<()> {
    /* Early exit for completions */
    match args.command {
        Command::Completions(completions) => {
            args::gen_completions(&completions)?;
            return Ok(());
        }
        _ => {}
    }

    let mut state = State::new();
    //let keycloak_glue = Keycloak::new().await?;
    let gitlab_glue = GitLabGlue::new().await?;

    //keycloak_glue.gather(&mut state).await?;
    gitlab_glue.gather(&mut state).await?;

    match args.command {
        Command::Completions(_) => {}
        Command::Keycloak(action) => {
            //keycloak_glue.run(&state, action).await?;
        }
        Command::Gitlab(action) => gitlab_glue.run(&state, action).await?,
        Command::Plan => {
            //keycloak_glue.run(&state, Action::Plan).await?;
            gitlab_glue.run(&state, Action::Plan).await?;
        }
        Command::Apply => {
            //keycloak_glue.run(&state, Action::Apply).await?;
            gitlab_glue.run(&state, Action::Apply).await?;
        }
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

    env_logger::init_from_env(Env::default().default_filter_or(logging));

    if let Err(err) = run(args).await {
        error!("Error: {:?}", err);
        for cause in err.chain() {
            error!("Caused by: {:?}", cause)
        }
        std::process::exit(1)
    }
}
