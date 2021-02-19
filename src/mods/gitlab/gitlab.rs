//! This module defines gitlab related actions and enforcements.
//!
//! ## Features
//!
//! - ensure the integrity of the Arch Linux root group
//!   - add staff members as reporter
//!   - ensure nobody except devops has higher privileges

use crate::args::Action;
use crate::state::State;

use crate::mods::gitlab::types::*;

use anyhow::{Context, Result};
use log::{debug, error, info};

use std::env;

use gitlab::api::{ApiError, Query};
use gitlab::Gitlab;

use gitlab::api::common::{AccessLevel, VisibilityLevel};
use gitlab::api::projects::{Projects, FeatureAccessLevel};
use gitlab::api::groups::members::{GroupMembers, AddGroupMember, RemoveGroupMember};
use tokio::task;
use std::sync::{Arc, Mutex};

const MAIN_BRANCH: &str = "main";
const ALL_TAGS: &str = "*";

pub struct GitLabGlue {
    client: Gitlab,
    state: Arc<Mutex<State>>,
}

impl GitLabGlue {
    pub async fn new(state: Arc<Mutex<State>>) -> Result<GitLabGlue> {
        create_client(state)
    }

    pub async fn gather(&self) -> Result<()> {
        Ok(())
    }

    pub async fn run<'a>(&self, action: Action) -> Result<()> {
        self.update_gitlab_group_members(&action).await?;
            /*
        task::spawn_blocking(move || {
            update_gitlab_group_members(guard, &action)
        }).await??;

             */
        //task::spawn_blocking(move || update_package_repositories(&action)).await??;
        Ok(())
    }

async fn update_gitlab_group_members(&self, action: &Action) -> Result<()> {
    let token =
        &env::var("GLUEBUDDY_GITLAB_TOKEN").context("Missing env var GLUEBUDDY_GITLAB_TOKEN")?;
    let client = Gitlab::new("gitlab.archlinux.org", token).unwrap();

    println!("members");

    let members_endpoint = gitlab::api::groups::members::GroupMembers::builder()
        .group("bot-test")
        .build()
        .unwrap();
    let members: Vec<GroupMember> = members_endpoint.query(&client).unwrap();
    for member in &members {
        println!("{} {} {} {}", member.id, member.username, member.email.as_ref().unwrap_or(&"-".to_string()), member.access_level);
    }

    let state = self.state.lock().unwrap();
    for staff in &state.staff {
        let member_names = members.iter().map(|e| e.username.clone()).collect::<Vec<_>>();
        let staff_username = &staff.username;
        if !member_names.contains(&staff_username) {
            println!("not in group: {}", staff_username);
        }
    }

    println!("project");

    let members_endpoint = gitlab::api::projects::members::ProjectMembers::builder()
        .project("archlinux/signstar")
        .build()
        .unwrap();
    let members: Vec<GroupMember> = members_endpoint.query(&client).unwrap();
    for member in members {
        println!("{} {} {} {}", member.id, member.username, member.email.as_ref().unwrap_or(&"-".to_string()), member.access_level);
    }

    println!("search");
    let users_endpoint = gitlab::api::users::Users::builder()
        .username("anthraxx")
        .active(())
        .external(false)
        .build()
        .unwrap();
    let users: Vec<User> = users_endpoint.query(&client).unwrap();
    for user in users {
        println!("{} {} {}", user.id, user.username, user.email.unwrap_or("-".to_string()));

        /*
        let endpoint = gitlab::api::groups::members::RemoveGroupMember::builder()
            .group("bot-test")
            .user(user.id)
            .build()
            .unwrap();
        gitlab::api::ignore(endpoint).query(&client).unwrap();

        let endpoint = gitlab::api::groups::members::AddGroupMember::builder()
            .group("bot-test")
            .user(user.id)
            .access_level(AccessLevel::Reporter)
            .build()
            .unwrap();
        gitlab::api::ignore(endpoint).query(&client).unwrap();
         */

        /*
        let endpoint = gitlab::api::projects::members::RemoveProjectMember::builder()
            .project("bot-test/sandbox/lib10000")
            .user(user.id)
            .build()
            .unwrap();
        gitlab::api::ignore(endpoint).query(&client).unwrap();

        let endpoint = gitlab::api::projects::members::AddProjectMember::builder()
            .project("bot-test/sandbox/lib10000")
            .user(user.id)
            .access_level(AccessLevel::Reporter)
            .build()
            .unwrap();
        gitlab::api::ignore(endpoint).query(&client).unwrap();

        let endpoint = gitlab::api::projects::members::EditProjectMember::builder()
            .project("bot-test/sandbox/lib10000")
            .user(user.id)
            .access_level(AccessLevel::Developer)
            .build()
            .unwrap();
        gitlab::api::ignore(endpoint).query(&client).unwrap();
         */
    }

    Ok(())
}
}

pub fn create_client(state: Arc<Mutex<State>>) -> Result<GitLabGlue> {
    let token =
        &env::var("GLUEBUDDY_GITLAB_TOKEN").context("Missing env var GLUEBUDDY_GITLAB_TOKEN")?;
    let client = Gitlab::new("gitlab.archlinux.org", token).unwrap();
    Ok(GitLabGlue { client, state })
}

fn update_package_repositories(action: &Action) -> Result<()> {
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

            let protected_tag = get_protected_tag(&client, &project, ALL_TAGS);

            match protected_tag {
                Ok(protected_tag) => {
                    debug!("protected tag {} exists", protected_tag.name);

                    println!("    protected-tag: {}", protected_tag.name);
                    for access_level in &protected_tag.create_access_levels {
                        println!(
                            "      create access -> {} {}",
                            access_level.access_level_description, access_level.access_level
                        )
                    }

                    let developer_has_create_access = protected_tag
                        .create_access_levels
                        .into_iter()
                        .any(|access| access.access_level == 30);

                    debug!("has create access: {}", developer_has_create_access);
                    if !developer_has_create_access {
                        debug!(">>> improper access level, re-protecting...");

                        if unprotect_tag(&client, &project, ALL_TAGS).is_err() {
                            eprintln!("Failed to unprotect tags for project {}", project.name);
                        }
                        if protect_tag(&client, &project, ALL_TAGS).is_err() {
                            eprintln!("Failed to protect tags for project {}", project.name);
                        }
                    }
                }
                Err(_) => {
                    if protect_tag(&client, &project, ALL_TAGS).is_err() {
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

            let protected_branch = get_protected_branch(&client, &project, MAIN_BRANCH);

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

                        if unprotect_branch(&client, &project, MAIN_BRANCH).is_err() {
                            eprintln!(
                                "Failed to unprotect branch main for project {}",
                                project.name
                            );
                        }
                        if protect_branch(&client, &project, MAIN_BRANCH).is_err() {
                            eprintln!("Failed to protect branch main for project {}", project.name);
                        }
                    }
                }
                Err(_) => {
                    if protect_branch(&client, &project, MAIN_BRANCH).is_err() {
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

            set_project_settings(&client, &project)?;
        }
    }

    Ok(())
}

fn set_project_settings(client: &Gitlab, project: &GroupProjects) -> Result<()> {
    if project.visibility == ProjectVisibilityLevel::Public
        && project.request_access_enabled == false
        && project.container_registry_enabled == false
        && project.snippets_access_level == ProjectFeatureAccessLevel::Disabled
    {
        return Ok(());
    }
    println!("edit project settings");
    let endpoint = gitlab::api::projects::EditProject::builder()
        .project(project.id)
        .visibility(VisibilityLevel::Public)
        .request_access_enabled(false)
        .container_registry_enabled(false)
        .snippets_access_level(FeatureAccessLevel::Disabled)
        .build()
        .unwrap();
    gitlab::api::ignore(endpoint).query(client).unwrap();
    Ok(())
}

fn get_protected_branch(
    client: &Gitlab,
    project: &GroupProjects,
    branch: &str,
) -> Result<ProtectedBranch> {
    let endpoint = gitlab::api::projects::protected_branches::ProtectedBranch::builder()
        .project(project.id)
        .name(branch)
        .build()
        .unwrap();
    let protected_branch: ProtectedBranch = endpoint.query(client)?;
    Ok(protected_branch)
}

fn protect_branch(
    client: &Gitlab,
    project: &GroupProjects,
    branch: &str,
) -> Result<ProtectedBranch> {
    // protect main branch
    let endpoint = gitlab::api::projects::protected_branches::ProtectBranch::builder()
        .project(project.id)
        .name(branch)
        .push_access_level(
            gitlab::api::projects::protected_branches::ProtectedAccessLevel::Developer,
        )
        .merge_access_level(
            gitlab::api::projects::protected_branches::ProtectedAccessLevel::Developer,
        )
        .build()
        .unwrap();
    let result: ProtectedBranch = endpoint.query(client)?;
    Ok(result)
}

fn unprotect_branch(client: &Gitlab, project: &GroupProjects, branch: &str) -> Result<()> {
    let endpoint = gitlab::api::projects::protected_branches::UnprotectBranch::builder()
        .project(project.id)
        .name(branch)
        .build()
        .unwrap();
    let _: () = gitlab::api::ignore(endpoint).query(client)?;
    Ok(())
}

fn get_protected_tag(client: &Gitlab, project: &GroupProjects, tag: &str) -> Result<ProtectedTag> {
    let endpoint = gitlab::api::projects::protected_tags::ProtectedTag::builder()
        .project(project.id)
        .name(tag)
        .build()
        .unwrap();
    let protected_tag: ProtectedTag = endpoint.query(client)?;
    Ok(protected_tag)
}

fn unprotect_tag(client: &Gitlab, project: &GroupProjects, tag: &str) -> Result<()> {
    let endpoint = gitlab::api::projects::protected_tags::UnprotectTag::builder()
        .project(project.id)
        .name(tag)
        .build()
        .unwrap();
    let _: () = gitlab::api::ignore(endpoint).query(client)?;
    Ok(())
}

fn protect_tag(client: &Gitlab, project: &GroupProjects, tag: &str) -> Result<ProtectedTag> {
    debug!("protecting tag *");
    let endpoint = gitlab::api::projects::protected_tags::ProtectTag::builder()
        .project(project.id)
        .name(tag)
        .create_access_level(gitlab::api::common::ProtectedAccessLevel::Developer)
        .build()
        .unwrap();
    let result: ProtectedTag = endpoint.query(client)?;
    Ok(result)
}
