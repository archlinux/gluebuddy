use clap::builder::styling;
use clap::{ArgAction, Args as ClapArgs, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

use std::io::stdout;

/// A secure helper daemon that watches several aspects
/// of the Arch Linux infrastructure and makes sure that certain conditions are met.
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None, styles=help_style())]
#[command(propagate_version = true)]
pub struct Args {
    /// Verbose logging, specify twice for more
    #[arg(short, long, action = ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
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
    pub shell: Shell,
}

#[derive(Debug, Subcommand)]
pub enum Action {
    /// Generate and show an execution plan
    Plan,

    /// Builds or changes infrastructure
    Apply,
}

pub fn gen_completions(completions: &Completions) {
    let mut cmd = Args::command();
    let bin_name = cmd.get_name().to_string();
    clap_complete::generate(completions.shell, &mut cmd, &bin_name, &mut stdout());
}

pub fn help_style() -> clap::builder::Styles {
    styling::Styles::styled()
        .usage(styling::AnsiColor::Green.on_default() | styling::Effects::BOLD)
        .header(styling::AnsiColor::Green.on_default() | styling::Effects::BOLD)
        .literal(styling::AnsiColor::Cyan.on_default() | styling::Effects::BOLD)
        .invalid(styling::AnsiColor::Yellow.on_default() | styling::Effects::BOLD)
        .error(styling::AnsiColor::Red.on_default() | styling::Effects::BOLD)
        .valid(styling::AnsiColor::Cyan.on_default() | styling::Effects::BOLD)
        .placeholder(styling::AnsiColor::Cyan.on_default())
}
