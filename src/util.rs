use crate::components::gitlab::types::{
    MyProtectedAccessLevel, ProjectFeatureAccessLevel, ProjectFeatureAccessLevelPublic,
    ProjectMergeMethod,
};
use anyhow::Result;
use difference::{Changeset, Difference};
use gitlab::api::common::AccessLevel;
use itertools::Itertools;

pub fn print_diff(text1: &str, text2: &str) -> Result<()> {
    let Changeset { diffs, .. } = Changeset::new(text1, text2, "\n");

    for diff in diffs {
        match diff {
            Difference::Same(ref x) => {
                for line in x.lines() {
                    let line = format!(" {}", line);
                    writeln(&line, None)?;
                }
            }
            Difference::Add(ref x) => {
                for line in x.lines() {
                    let line = format!("+{}", line);
                    writeln(&line, Some(term::color::GREEN))?;
                }
            }
            Difference::Rem(ref x) => {
                for line in x.lines() {
                    let line = format!("-{}", line);
                    writeln(&line, Some(term::color::RED))?;
                }
            }
        }
    }

    Ok(())
}

pub fn writeln(line: &str, fg: Option<term::color::Color>) -> Result<()> {
    let stdout = term::stdout();

    match stdout {
        Some(mut stdout) => {
            match fg {
                Some(color) => stdout.fg(color)?,
                None => stdout.reset()?,
            }
            writeln!(stdout, "{}", line)?;

            stdout.reset()?;
            stdout.flush()?;
        }
        None => {
            println!("{}", line);
        }
    }

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

pub fn format_mailman_membership(mailing_list: &str, email: &str) -> String {
    format!(
        "mailing_list_membership {{\n\tmailing_list = {}\n\temail  = {}\n\n}}",
        mailing_list, email
    )
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

#[allow(clippy::too_many_arguments)]
pub fn format_gitlab_project_settings(
    namespace: &str,
    description: &str,
    request_access_enabled: bool,
    issues_access_level: ProjectFeatureAccessLevel,
    merge_requests_access_level: ProjectFeatureAccessLevel,
    merge_method: ProjectMergeMethod,
    only_allow_merge_if_all_discussions_are_resolved: bool,
    builds_access_level: ProjectFeatureAccessLevel,
    container_registry_access_level: ProjectFeatureAccessLevel,
    packages_enabled: bool,
    snippets_access_level: ProjectFeatureAccessLevel,
    lfs_enabled: bool,
    service_desk_enabled: bool,
    pages_access_level: ProjectFeatureAccessLevelPublic,
    requirements_access_level: ProjectFeatureAccessLevel,
    releases_access_level: ProjectFeatureAccessLevel,
    environments_access_level: ProjectFeatureAccessLevel,
    feature_flags_access_level: ProjectFeatureAccessLevel,
    infrastructure_access_level: ProjectFeatureAccessLevel,
    monitor_access_level: ProjectFeatureAccessLevel,
) -> String {
    format!(
        "gitlab_project_setting {{\n\
        \tnamespace                       = {}\n\
        \tdescription                     = {}\n\
        \trequest_access_enabled          = {}\n\
        \tissues_access_level             = {}\n\
        \tmerge_requests_access_level     = {}\n\
        \tmerge_method                    = {}\n\
        \tonly_allow_merge_if_all_discussions_are_resolved = {}\n\
        \tbuilds_access_level             = {}\n\
        \tcontainer_registry_access_level = {}\n\
        \tpackages_enabled                = {}\n\
        \tsnippets_access_level           = {}\n\
        \tlfs_enabled                     = {}\n\
        \tservice_desk_enabled            = {}\n\
        \tpages_access_level              = {}\n\
        \trequirements_access_level       = {}\n\
        \treleases_access_level           = {}\n\
        \tenvironments_access_level       = {}\n\
        \tfeature_flags_access_level      = {}\n\
        \tinfrastructure_access_level     = {}\n\
        \tmonitor_access_level            = {}\n\
        }}",
        namespace,
        description.replace('\n', "\\n"),
        request_access_enabled,
        issues_access_level.as_str(),
        merge_requests_access_level.as_str(),
        merge_method.as_str(),
        only_allow_merge_if_all_discussions_are_resolved,
        builds_access_level.as_str(),
        container_registry_access_level.as_str(),
        packages_enabled,
        snippets_access_level.as_str(),
        lfs_enabled,
        service_desk_enabled,
        pages_access_level.as_str(),
        requirements_access_level.as_str(),
        releases_access_level.as_str(),
        environments_access_level.as_str(),
        feature_flags_access_level.as_str(),
        infrastructure_access_level.as_str(),
        monitor_access_level.as_str(),
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

pub fn format_gitlab_project_protected_tag(
    namespace: &str,
    name: &str,
    create_access_level: &[MyProtectedAccessLevel],
) -> String {
    format!(
        "gitlab_project_protected_tag {{\n\
        \tnamespace           = {}\n\
        \tname                = {}\n\
        \tcreate_access_level = {}\n\
        }}",
        namespace,
        name,
        create_access_level
            .iter()
            .map(|access| access.as_str())
            .join(", ")
    )
}

pub fn format_separator() -> String {
    "-".repeat(72)
}
