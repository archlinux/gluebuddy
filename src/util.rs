use crate::components::gitlab::types::ProjectFeatureAccessLevel;
use anyhow::{Context, Result};
use difference::{Changeset, Difference};
use gitlab::api::common::AccessLevel;

pub fn print_diff(text1: &str, text2: &str) -> Result<()> {
    let Changeset { diffs, .. } = Changeset::new(text1, text2, "\n");

    let mut stdout = term::stdout().context("failed to get stdout")?;

    for diff in diffs {
        match diff {
            Difference::Same(ref x) => {
                for line in x.lines() {
                    stdout.reset()?;
                    writeln!(stdout, " {}", line)?;
                }
            }
            Difference::Add(ref x) => {
                for line in x.lines() {
                    stdout.fg(term::color::GREEN)?;
                    writeln!(stdout, "+{}", line)?;
                }
            }
            Difference::Rem(ref x) => {
                for line in x.lines() {
                    stdout.fg(term::color::RED)?;
                    writeln!(stdout, "-{}", line)?;
                }
            }
        }
    }

    stdout.reset()?;
    stdout.flush()?;

    Ok(())
}

pub fn access_level_from_u64(access_level: u64) -> AccessLevel {
    match access_level {
        60 => AccessLevel::Admin,
        50 => AccessLevel::Owner,
        40 => AccessLevel::Maintainer,
        30 => AccessLevel::Developer,
        20 => AccessLevel::Reporter,
        10 => AccessLevel::Guest,
        5 => AccessLevel::Minimal,
        _ => AccessLevel::Anonymous,
    }
}

pub fn format_gitlab_member_access(
    namespace: &str,
    username: &str,
    access_level: AccessLevel,
) -> String {
    format!(
        "gitlab_member_access {{\n\
        \tnamespace    = {}\n\
        \tusername     = {}\n\
        \taccess_level = {}\n\
        }}",
        namespace,
        username,
        access_level.as_str()
    )
}

pub fn format_gitlab_user(username: &str, admin: bool) -> String {
    format!(
        "gitlab_user {{\n\
        \tusername = {}\n\
        \tadmin    = {}\n\
        }}",
        username, admin,
    )
}

pub fn format_gitlab_project_settings(
    namespace: &str,
    request_access_enabled: bool,
    snippets_access_level: ProjectFeatureAccessLevel,
) -> String {
    format!(
        "gitlab_project_setting {{\n\
        \tnamespace              = {}\n\
        \trequest_access_enabled = {}\n\
        \tsnippets_access_level  = {}\n\
        }}",
        namespace,
        request_access_enabled,
        snippets_access_level.as_str(),
    )
}

pub fn format_gitlab_group_settings(path: &str, request_access_enabled: bool) -> String {
    format!(
        "gitlab_group_setting {{\n\
        \tnamespace              = {}\n\
        \trequest_access_enabled = {}\n\
        }}",
        path, request_access_enabled,
    )
}

pub fn format_separator() -> String {
    "-".repeat(72)
}
