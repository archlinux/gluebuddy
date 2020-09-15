//! This module defines gitlab related actions and enforcements.
//!
//! ## Features
//!
//! - ensure the integrity of the Arch Linux root group
//!   - add staff members as reporter
//!   - ensure nobody except devops has higher privileges


use crate::args::*;
use crate::state::State;

mod types;
use types::*;

use anyhow::{Context, Result};
use log::{debug, error, info};

use std::env;

use gitlab::api::{ApiError, Query};
use gitlab::Gitlab;

use gitlab::api::common::VisibilityLevel;
use gitlab::api::projects::Projects;
use tokio::task;

pub struct GitLabGlue {
    client: Gitlab,
}

impl GitLabGlue {
    pub async fn new() -> Result<GitLabGlue> {
        task::spawn_blocking(move || create_client()).await?
    }

    pub async fn gather<'a>(&'a self, state: &mut State<'a>) -> Result<()> {
        Ok(())
    }

    pub async fn run<'a>(&self, state: &State<'a>, action: Action) -> Result<()> {
        task::spawn_blocking(move || create_repository(action)).await??;
        Ok(())
    }
}

pub fn create_client() -> Result<GitLabGlue> {
    let token =
        &env::var("GLUEBUDDY_GITLAB_TOKEN").context("Missing env var GLUEBUDDY_GITLAB_TOKEN")?;
    let client = Gitlab::new("gitlab.archlinux.org", token).unwrap();
    Ok(GitLabGlue { client })
}

/*
fn protect_tags(client: &Gitlab, project_id: u64) -> std::result::Result<ProtectedTag, gitlab::api::ApiError<>> {
}
 */

fn create_repository(action: Action) -> Result<()> {
    let token =
        &env::var("GLUEBUDDY_GITLAB_TOKEN").context("Missing env var GLUEBUDDY_GITLAB_TOKEN")?;
    let client = Gitlab::new("gitlab.archlinux.org", token).unwrap();

    let group_endpoint = gitlab::api::groups::subgroups::GroupSubgroups::builder()
        .group("bot-test")
        .build()
        .unwrap();
    let groups: Vec<Group> = group_endpoint.query(&client).unwrap();

    for group in groups {
        println!("group: {}", group.name);

        // TODO: remove this
        if !group.name.eq("sandbox") {
            continue;
        }

        let group_projects_endpoint = gitlab::api::groups::projects::GroupProjects::builder()
            .group(group.id)
            .archived(false)
            .order_by(gitlab::api::groups::projects::GroupProjectsOrderBy::Id)
            .build()
            .unwrap();
        let projects: Vec<GroupProjects> =
            gitlab::api::paged(group_projects_endpoint, gitlab::api::Pagination::All)
                .query(&client)?;

        for project in projects {
            println!("  project: {}", project.name);

            let endpoint = gitlab::api::projects::protected_tags::ProtectedTag::builder()
                .project(project.id)
                .name("*")
                .build()
                .unwrap();
            let protected_tag: Result<ProtectedTag, gitlab::api::ApiError<_>> =
                endpoint.query(&client);

            match protected_tag {
                Ok(protected_tag) => {
                    debug!("protected tag {} exists", protected_tag.name);

                    let developer_has_create_access = protected_tag
                        .create_access_levels
                        .into_iter()
                        .any(|access| access.access_level == 30);

                    debug!("has create access: {}", developer_has_create_access);
                    if !developer_has_create_access {
                        debug!(">>> improper access level, re-protecting...");

                        if unprotect_tags(&client, &project).is_err() {
                            eprintln!("Failed to unprotect tags for project {}", project.name);
                        }
                        if protect_tags(&client, &project).is_err() {
                            eprintln!("Failed to protect tags for project {}", project.name);
                        }
                    }
                }
                Err(_) => {
                    if protect_tags(&client, &project).is_err() {
                        eprintln!("Failed to protect tags for project {}", project.name);
                    }
                }
            }

            /* This is just debug printing
            let protected_tag: ProtectedTag =
                gitlab::api::projects::protected_tags::ProtectedTag::builder()
                    .project(project.id)
                    .name("*")
                    .build()
                    .unwrap()
                    .query(&client)
                    .unwrap();
            println!("    protected-tag: {}", protected_tag.name);
            for access_level in protected_tag.create_access_levels {
                println!(
                    "      create access -> {} {}",
                    access_level.access_level_description, access_level.access_level
                )
            }
            */

            let protected_branch: Result<ProtectedBranch, _> =
                gitlab::api::projects::protected_branches::ProtectedBranch::builder()
                    .project(project.id)
                    .name("main")
                    .build()
                    .unwrap()
                    .query(&client);
            match protected_branch {
                Ok(protected_branch) => {
                    debug!("protection for branch {} exists", protected_branch.name);
                    println!("    protected-branch: {}", protected_branch.name);
                    for access_level in &protected_branch.push_access_levels {
                        println!(
                            "      push access -> {}",
                            access_level.access_level_description
                        )
                    }
                    for access_level in &protected_branch.merge_access_levels {
                        println!(
                            "      merge access -> {}",
                            access_level.access_level_description
                        )
                    }

                    let developer_has_push_access = protected_branch
                        .push_access_levels
                        .into_iter()
                        .any(|access| access.access_level == 30);
                    let developer_has_merge_access = protected_branch
                        .merge_access_levels
                        .into_iter()
                        .any(|access| access.access_level == 30);

                    debug!(
                        "has push access: {} has merge access: {}",
                        developer_has_push_access, developer_has_merge_access
                    );
                    if !developer_has_merge_access || !developer_has_push_access {
                        println!(">>> improper access level, re-protecting...");

                        if unprotect_branch(&client, &project).is_err() {
                            eprintln!("Failed to unprotect branch main for project {}", project.name);
                        }
                        if protect_branch(&client, &project).is_err() {
                            eprintln!("Failed to protect branch main for project {}", project.name);
                        }
                    }
                }
                Err(_) => {
                    if protect_branch(&client, &project).is_err() {
                        eprintln!("Failed to protect branch main for project {}", project.name);
                    }
                }
            }

            /* just debug output printing
            let protected_branch: ProtectedBranch =
                gitlab::api::projects::protected_branches::ProtectedBranch::builder()
                    .project(project.id)
                    .name("main")
                    .build()
                    .unwrap()
                    .query(&client)
                    .unwrap();
            println!("    protected-branch: {}", protected_branch.name);
            for access_level in protected_branch.push_access_levels {
                println!(
                    "      push access -> {}",
                    access_level.access_level_description
                )
            }
            for access_level in protected_branch.merge_access_levels {
                println!(
                    "      merge access -> {}",
                    access_level.access_level_description
                )
            }
             */

            if project.visibility != ProjectVisibilityLevel::Public
                || project.request_access_enabled != false
            {
                println!("      edit project settings");
                let endpoint = gitlab::api::projects::EditProject::builder()
                    .project(project.id)
                    .visibility(VisibilityLevel::Public)
                    .request_access_enabled(false)
                    .build()
                    .unwrap();
                gitlab::api::ignore(endpoint).query(&client).unwrap();
            }
        }
    }

    Ok(())
}

fn unprotect_tags(client: &Gitlab, project: &GroupProjects) -> Result<()> {
    let endpoint =
        gitlab::api::projects::protected_tags::UnprotectTag::builder()
            .project(project.id)
            .name("*")
            .build()
            .unwrap();
    let _: () = gitlab::api::ignore(endpoint).query(client)?;
    Ok(())
}

fn protect_tags(client: &Gitlab, project: &GroupProjects) -> Result<ProtectedTag> {
    debug!("protecting tag *");
    let endpoint = gitlab::api::projects::protected_tags::ProtectTag::builder()
        .project(project.id)
        .name("*")
        .create_access_level(gitlab::api::common::ProtectedAccessLevel::Developer)
        .build()
        .unwrap();
    let result: ProtectedTag = endpoint.query(client)?;
    Ok(result)
}

fn protect_branch(client: &Gitlab, project: &GroupProjects) -> Result<ProtectedBranch> {
    // protect main branch
    let endpoint = gitlab::api::projects::protected_branches::ProtectBranch::builder().project(project.id).name("main")
        .push_access_level(gitlab::api::projects::protected_branches::ProtectedAccessLevel::Developer)
        .merge_access_level(gitlab::api::projects::protected_branches::ProtectedAccessLevel::Developer)
        .build().unwrap();
    let result: ProtectedBranch = endpoint.query(client)?;
    Ok(result)
}

fn unprotect_branch(client: &Gitlab, project: &GroupProjects) -> Result<()> {
    let endpoint =
        gitlab::api::projects::protected_branches::UnprotectBranch::builder()
            .project(project.id)
            .name("main")
            .build()
            .unwrap();
    let _: () = gitlab::api::ignore(endpoint).query(client)?;
    Ok(())
}
