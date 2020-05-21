use structopt::StructOpt;
use structopt::clap::{AppSettings, Shell};

use std::io::stdout;

use anyhow::Result;

#[derive(Debug, StructOpt)]
#[structopt(about="A secure helper daemon that watches several aspects of the Arch Linux infrastructure and makes sure that certain conditions are met.", global_settings = &[AppSettings::ColoredHelp, AppSettings::DeriveDisplayOrder])]
pub struct Args {
    /// Verbose logging, specify twice for more
    #[structopt(short, long, parse(from_occurrences))]
    pub verbose: u8,
    #[structopt(subcommand)]
    pub subcommand: Option<SubCommand>,
}

#[derive(Debug, StructOpt)]
pub enum SubCommand {
    /// Generate shell completions
    #[structopt(name="completions")]
    Completions(Completions),
}

#[derive(Debug, StructOpt)]
pub struct Completions {
    #[structopt(possible_values=&Shell::variants())]
    pub shell: Shell,
}

pub fn gen_completions(args: &Completions) -> Result<()> {
    Args::clap().gen_completions_to("gluebuddy", args.shell, &mut stdout());
    Ok(())
}
