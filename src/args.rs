use clap::{AppSettings, Args as ClapArgs, IntoApp, Parser, Subcommand};
use clap_complete::Shell;

use std::io::stdout;

use anyhow::Result;

/// A secure helper daemon that watches several aspects
/// of the Arch Linux infrastructure and makes sure that certain conditions are met.
#[derive(Debug, Parser)]
#[clap(version, global_setting = AppSettings::DeriveDisplayOrder)]
pub struct Args {
    /// Verbose logging, specify twice for more
    #[clap(short, long, parse(from_occurrences))]
    pub verbose: u8,

    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Generate and show an execution plan
    Plan,

    /// Builds or changes infrastructure
    Apply,

    /// Keycloak module commands
    Keycloak {
        #[clap(subcommand)]
        action: Action,
    },

    /// Gitlab module commands
    Gitlab {
        #[clap(subcommand)]
        action: Action,
    },

    /// Generate shell completions
    #[clap(name = "completions")]
    Completions(Completions),
}

#[derive(Debug, ClapArgs)]
pub struct Completions {
    /// Target shell
    #[clap(arg_enum)]
    pub shell: Shell,
}

#[derive(Debug, Subcommand)]
pub enum Action {
    /// Generate and show an execution plan
    Plan,

    /// Builds or changes infrastructure
    Apply,
}

pub fn gen_completions(args: &Completions) -> Result<()> {
    clap_complete::generate(
        args.shell,
        &mut Args::into_app(),
        "gluebuddy",
        &mut stdout(),
    );
    Ok(())
}
