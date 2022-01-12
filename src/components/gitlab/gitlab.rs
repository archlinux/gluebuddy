//! This module defines gitlab related actions and enforcements.
//!
//! ## Features
//!
//! - ensure the integrity of the Arch Linux root group
//!   - add staff members with minimal access
//!   - ensure nobody except devops has higher privileges

use crate::args::Action;
use crate::state::{State, User};

use crate::components::gitlab::types::*;

use crate::util;

use std::env;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use log::{debug, error, info, trace, warn};
use tokio::sync::{Mutex, MutexGuard};

use gitlab::api::{AsyncQuery, Query};
use gitlab::{AsyncGitlab, Gitlab, GitlabBuilder};

use gitlab::api::common::AccessLevel;
use gitlab::api::groups::projects::GroupProjectsOrderBy;
use gitlab::api::groups::subgroups::GroupSubgroupsOrderBy;
use gitlab::api::users::ExternalProvider;

const DEFAULT_ARCH_LINUX_GROUP_ACCESS_LEVEL: AccessLevel = AccessLevel::Minimal;
const DEFAULT_STAFF_GROUP_ACCESS_LEVEL: AccessLevel = AccessLevel::Reporter;
const DEVOPS_INFRASTRUCTURE_ACCESS_LEVEL: AccessLevel = AccessLevel::Developer;
const MAX_ACCESS_LEVEL: AccessLevel = AccessLevel::Developer;

const GITLAB_OWNER: &str = "archceo";
const GITLAB_BOT: &str = "archbot";

const MAIN_BRANCH: &str = "main";
const ALL_TAGS: &str = "*";

pub struct GitLabGlue {
    client: AsyncGitlab,
    state: Arc<Mutex<State>>,
}

impl GitLabGlue {
    pub async fn new(state: Arc<Mutex<State>>) -> Result<GitLabGlue> {
        let token = &env::var("GLUEBUDDY_GITLAB_TOKEN")
            .context("Missing env var GLUEBUDDY_GITLAB_TOKEN")?;
        let client = GitlabBuilder::new("gitlab.archlinux.org", token)
            .build_async()
            .await?;
        Ok(GitLabGlue { client, state })
    }

    pub async fn gather(&self) -> Result<()> {
        self.gather_gitlab_user_ids().await?;
        Ok(())
    }

    pub async fn gather_gitlab_user_ids(&self) -> Result<()> {
        info!("Gathering GitLab state");
        let mut state = self.state.lock().await;
        for user in &mut state.users.values_mut() {
            let username = &user.username;
            let users_endpoint = gitlab::api::users::Users::builder()
                .external_provider(
                    ExternalProvider::builder()
                        .uid(username)
                        .name("saml")
                        .build()
                        .unwrap(),
                )
                .active(())
                .external(false)
                .build()
                .unwrap();
            let users: Vec<GitLabUser> = users_endpoint.query_async(&self.client).await?;
            if users.is_empty() {
                warn!("Failed to query GitLab user for {}", username);
                continue;
            } else if users.len() > 1 {
                bail!(
                    "Somehow got {} GitLab user results for {}",
                    users.len(),
                    username
                )
            }
            let gitlab_user = users
                .first()
                .with_context(|| format!("Failed to query GitLab user for {}", username))?;
            debug!(
                "Successfully retrieved user {} to GitLab id {}",
                gitlab_user.username, gitlab_user.id
            );
            if user.username != gitlab_user.username {
                error!(
                    "Username mismatch between keycloak and GitLab: {} vs {}",
                    user.username, gitlab_user.username
                );
            }
            user.gitlab_id = Some(gitlab_user.id);
        }

        Ok(())
    }

    pub async fn run(&self, action: Action) -> Result<()> {
        self.update_archlinux_group_recursively(&action).await?;
        self.update_archlinux_group_members(&action).await?;
        self.update_staff_group_members(&action).await?;
        self.update_devops_group_members(&action).await?;
        self.update_infrastructure_project_members(&action).await?;
        Ok(())
    }

    async fn update_archlinux_group_recursively(&self, action: &Action) -> Result<()> {
        let group = "archlinux";
        let endpoint = gitlab::api::groups::Group::builder()
            .group(group)
            .build()
            .unwrap();
        let root: Group = endpoint.query_async(&self.client).await?;

        let mut to_visit = vec![root];

        let state = self.state.lock().await;

        while !to_visit.is_empty() {
            match to_visit.pop() {
                None => {}
                Some(group) => {
                    let subgroups = self.get_group_subgroups(&group.full_path).await?;
                    for subgroup in subgroups {
                        to_visit.push(subgroup);
                    }

                    // TODO: disable request_access_enabled, API is missing to edit

                    let label = format!("GitLab '{}' group members", group.full_name);
                    let mut summary = PlanSummary::new(&label);
                    let members = self.get_group_members(&group.full_path).await?;
                    for member in &members {
                        if is_archlinux_bot(&member) {
                            continue;
                        }

                        match state.staff_from_gitlab_id(member.id) {
                            None => {
                                if self
                                    .remove_group_member(action, &state, member, &group.full_path)
                                    .await?
                                {
                                    summary.destroy += 1;
                                }
                            }
                            Some(user) => {
                                if self
                                    .edit_group_member_max_access_level(
                                        action,
                                        user,
                                        member,
                                        &group.full_path,
                                        MAX_ACCESS_LEVEL,
                                    )
                                    .await?
                                {
                                    summary.change += 1;
                                }
                            }
                        }
                    }

                    println!("{}", summary);
                    println!("{}", util::format_separator());

                    let projects = self.get_group_projects(&group.full_path).await?;
                    for project in projects {
                        let label =
                            format!("GitLab '{}' project settings", project.name_with_namespace);
                        let mut summary = PlanSummary::new(&label);

                        match self.apply_project_settings(action, &project).await? {
                            false => {}
                            true => {
                                summary.change += 1;
                            }
                        }

                        println!("{}", summary);
                        println!("{}", util::format_separator());

                        let label =
                            format!("GitLab '{}' project members", project.name_with_namespace);
                        let mut summary = PlanSummary::new(&label);
                        let members = self
                            .get_project_members(&project.path_with_namespace)
                            .await?;

                        for member in &members {
                            if is_archlinux_bot(&member) {
                                continue;
                            }

                            match state.staff_with_externals_from_gitlab_id(member.id) {
                                None => {
                                    if self
                                        .remove_project_member(
                                            action,
                                            member,
                                            &project.path_with_namespace,
                                        )
                                        .await?
                                    {
                                        summary.destroy += 1;
                                    }
                                }
                                Some(user) => {
                                    if self
                                        .edit_project_member_max_access_level(
                                            action,
                                            user,
                                            member,
                                            &project.path_with_namespace,
                                            MAX_ACCESS_LEVEL,
                                        )
                                        .await?
                                    {
                                        summary.change += 1;
                                    }
                                }
                            }
                        }

                        println!("{}", summary);
                        println!("{}", util::format_separator());
                    }
                }
            }
        }

        Ok(())
    }

    async fn update_archlinux_group_members(&self, action: &Action) -> Result<()> {
        let group = "archlinux";
        let archlinux_group_members = self.get_group_members(group).await?;

        let mut summary = PlanSummary::new("GitLab 'Arch Linux' group members");
        let state = self.state.lock().await;

        let gitlab_group_member_names = archlinux_group_members
            .iter()
            .map(|e| e.username.clone())
            .collect::<Vec<_>>();

        for staff in state.staff() {
            if !gitlab_group_member_names.contains(&staff.username) {
                if self
                    .add_group_member(action, &staff, group, DEFAULT_ARCH_LINUX_GROUP_ACCESS_LEVEL)
                    .await?
                {
                    summary.add += 1;
                }
            }
        }

        for member in &archlinux_group_members {
            if is_archlinux_bot(&member) {
                continue;
            }
            match state.staff_from_gitlab_id(member.id) {
                None => {
                    if self
                        .remove_group_member(action, &state, member, group)
                        .await?
                    {
                        summary.destroy += 1;
                    }
                }
                Some(user) => {
                    if self
                        .edit_group_member_access_level(
                            action,
                            user,
                            member,
                            group,
                            DEFAULT_ARCH_LINUX_GROUP_ACCESS_LEVEL,
                        )
                        .await?
                    {
                        summary.change += 1;
                    }
                }
            }
        }

        println!("{}", summary);
        println!("{}", util::format_separator());

        Ok(())
    }

    async fn update_staff_group_members(&self, action: &Action) -> Result<()> {
        let group = "archlinux/teams/staff";
        let archlinux_group_members = self.get_group_members(group).await?;

        let mut summary = PlanSummary::new("GitLab 'Arch Linux/Teams/Staff' group members");
        let state = self.state.lock().await;

        let gitlab_group_member_names = archlinux_group_members
            .iter()
            .map(|e| e.username.clone())
            .collect::<Vec<_>>();

        for staff in state.staff() {
            if !gitlab_group_member_names.contains(&staff.username) {
                if self
                    .add_group_member(action, &staff, group, DEFAULT_STAFF_GROUP_ACCESS_LEVEL)
                    .await?
                {
                    summary.add += 1;
                }
            }
        }

        for member in &archlinux_group_members {
            if is_archlinux_bot(&member) {
                continue;
            }
            match state.staff_from_gitlab_id(member.id) {
                None => {
                    if self
                        .remove_group_member(action, &state, member, group)
                        .await?
                    {
                        summary.destroy += 1;
                    }
                }
                Some(user) => {
                    if self
                        .edit_group_member_access_level(
                            action,
                            user,
                            member,
                            group,
                            DEFAULT_STAFF_GROUP_ACCESS_LEVEL,
                        )
                        .await?
                    {
                        summary.change += 1;
                    }
                }
            }
        }

        println!("{}", summary);
        println!("{}", util::format_separator());

        Ok(())
    }

    async fn update_devops_group_members(&self, action: &Action) -> Result<()> {
        let mut summary = PlanSummary::new("GitLab 'Arch Linux/Teams/DevOps' group members");
        let devops_group = "archlinux/teams/devops";
        let group_members = self.get_group_members(devops_group).await?;

        let group_member_names = group_members
            .iter()
            .map(|e| e.username.clone())
            .collect::<Vec<_>>();

        let state = self.state.lock().await;
        for staff in state.devops() {
            if !group_member_names.contains(&staff.username) {
                if self
                    .add_group_member(
                        action,
                        &staff,
                        devops_group,
                        DEVOPS_INFRASTRUCTURE_ACCESS_LEVEL,
                    )
                    .await?
                {
                    summary.add += 1;
                }
            }
        }

        for member in &group_members {
            if is_archlinux_bot(&member) {
                continue;
            }
            match state.devops_from_gitlab_id(member.id) {
                None => {
                    if self
                        .remove_group_member(action, &state, member, devops_group)
                        .await?
                    {
                        summary.destroy += 1;
                    }
                }
                Some(user) => match util::access_level_from_u64(member.access_level) {
                    DEVOPS_INFRASTRUCTURE_ACCESS_LEVEL => {}
                    _ => {
                        if self
                            .edit_group_member_access_level(
                                action,
                                user,
                                member,
                                devops_group,
                                DEVOPS_INFRASTRUCTURE_ACCESS_LEVEL,
                            )
                            .await?
                        {
                            summary.change += 1;
                        }
                    }
                },
            }
        }

        println!("{}", summary);
        println!("{}", util::format_separator());

        Ok(())
    }

    async fn update_infrastructure_project_members(&self, action: &Action) -> Result<()> {
        let project = "archlinux/infrastructure";
        let project_members = self.get_project_members(project).await?;

        let mut summary = PlanSummary::new("GitLab 'Arch Linux/Infrastructure' project members");

        for member in &project_members {
            if self.remove_project_member(action, member, project).await? {
                summary.destroy += 1;
            }
        }

        println!("{}", summary);
        println!("{}", util::format_separator());

        Ok(())
    }

    async fn get_group_members(&self, group: &str) -> Result<Vec<GitLabMember>> {
        let members_endpoint = gitlab::api::groups::members::GroupMembers::builder()
            .group(group)
            .build()
            .unwrap();
        let gitlab_group_members: Vec<GitLabMember> =
            gitlab::api::paged(members_endpoint, gitlab::api::Pagination::All)
                .query_async(&self.client)
                .await?;
        Ok(gitlab_group_members)
    }

    async fn get_project_members(&self, project: &str) -> Result<Vec<GitLabMember>> {
        let endpoint = gitlab::api::projects::members::ProjectMembers::builder()
            .project(project)
            .build()
            .unwrap();
        let members: Vec<GitLabMember> = gitlab::api::paged(endpoint, gitlab::api::Pagination::All)
            .query_async(&self.client)
            .await?;
        Ok(members)
    }

    async fn get_group_subgroups(&self, group: &str) -> Result<Vec<Group>> {
        let endpoint = gitlab::api::groups::subgroups::GroupSubgroups::builder()
            .group(group)
            .order_by(GroupSubgroupsOrderBy::Path)
            .build()
            .unwrap();
        let subgroups: Vec<Group> = gitlab::api::paged(endpoint, gitlab::api::Pagination::All)
            .query_async(&self.client)
            .await?;
        Ok(subgroups)
    }

    async fn get_group_projects(&self, group: &str) -> Result<Vec<GroupProjects>> {
        let endpoint = gitlab::api::groups::projects::GroupProjects::builder()
            .group(group)
            .order_by(GroupProjectsOrderBy::Path)
            .build()
            .unwrap();
        let projects: Vec<GroupProjects> =
            gitlab::api::paged(endpoint, gitlab::api::Pagination::All)
                .query_async(&self.client)
                .await?;
        Ok(projects)
    }

    async fn add_group_member(
        &self,
        action: &Action,
        user: &User,
        group: &str,
        access_level: AccessLevel,
    ) -> Result<bool> {
        let staff_username = &user.username;
        if user.gitlab_id.is_none() {
            debug!(
                "Skip adding {} to GitLab group: no GitLab user found",
                staff_username
            );
            return Ok(false);
        }
        let gitlab_id = user
            .gitlab_id
            .with_context(|| format!("Failed to unwrap GitLab user for {}", staff_username))?;

        debug!("Adding user {} to GitLab group '{}'", user.username, group);
        util::print_diff(
            &"",
            util::format_gitlab_member_access(group, &user.username, access_level).as_str(),
        )?;
        match action {
            Action::Apply => {
                let endpoint = gitlab::api::groups::members::AddGroupMember::builder()
                    .group(group)
                    .user(gitlab_id)
                    .access_level(access_level)
                    .build()
                    .unwrap();
                gitlab::api::ignore(endpoint)
                    .query_async(&self.client)
                    .await
                    .unwrap();
            }
            _ => {}
        }
        Ok(true)
    }

    async fn remove_group_member<'a>(
        &self,
        action: &Action,
        _state: &MutexGuard<'a, State>,
        member: &GitLabMember,
        group: &str,
    ) -> Result<bool> {
        debug!("User {} must not be in group {}", &member.username, group);
        util::print_diff(
            util::format_gitlab_member_access(
                group,
                &member.username,
                util::access_level_from_u64(member.access_level),
            )
            .as_str(),
            &"",
        )?;
        match action {
            Action::Apply => {
                let endpoint = gitlab::api::groups::members::RemoveGroupMember::builder()
                    .group(group)
                    .user(member.id)
                    .build()
                    .unwrap();
                gitlab::api::ignore(endpoint)
                    .query_async(&self.client)
                    .await
                    .unwrap();
            }
            _ => {}
        }
        Ok(true)
    }

    async fn edit_group_member_access_level<'a>(
        &self,
        action: &Action,
        user: &User,
        group_member: &GitLabMember,
        group: &str,
        expected_access_level: AccessLevel,
    ) -> Result<bool> {
        let access_level = util::access_level_from_u64(group_member.access_level);
        if access_level.eq(&expected_access_level) {
            trace!(
                "User {} has expected access_level {} in group {}",
                user.username,
                expected_access_level.as_str(),
                group,
            );
            return Ok(false);
        }

        debug!(
            "User {} should have access_level {} instead of {} in group {}",
            user.username,
            expected_access_level.as_str(),
            access_level.as_str(),
            group,
        );
        util::print_diff(
            util::format_gitlab_member_access(group, &user.username, access_level).as_str(),
            util::format_gitlab_member_access(group, &user.username, expected_access_level)
                .as_str(),
        )?;
        match action {
            Action::Apply => {
                let endpoint = gitlab::api::groups::members::EditGroupMember::builder()
                    .group(group)
                    .user(group_member.id)
                    .access_level(expected_access_level)
                    .build()
                    .unwrap();
                gitlab::api::ignore(endpoint)
                    .query_async(&self.client)
                    .await
                    .unwrap();
            }
            _ => {}
        }
        Ok(true)
    }

    async fn edit_group_member_max_access_level<'a>(
        &self,
        action: &Action,
        user: &User,
        group_member: &GitLabMember,
        group: &str,
        max_access_level: AccessLevel,
    ) -> Result<bool> {
        let access_level = util::access_level_from_u64(group_member.access_level);
        if max_access_level.as_u64() >= access_level.as_u64() {
            debug!(
                "User {} has access_level {} which is below max access_level {} in group {}",
                user.username,
                access_level.as_str(),
                max_access_level.as_str(),
                group,
            );
            return Ok(false);
        }

        self.edit_group_member_access_level(action, user, group_member, group, max_access_level)
            .await
    }

    async fn add_project_member(
        &self,
        action: &Action,
        user: &User,
        project: &str,
        access_level: AccessLevel,
    ) -> Result<bool> {
        let staff_username = &user.username;
        if user.gitlab_id.is_none() {
            warn!(
                "Skip adding {} to GitLab project: no GitLab user found",
                staff_username
            );
            return Ok(false);
        }
        let gitlab_id = user
            .gitlab_id
            .with_context(|| format!("Failed to unwrap GitLab user for {}", staff_username))?;

        debug!(
            "Adding user {} to GitLab project '{}'",
            user.username, project
        );
        util::print_diff(
            &"",
            util::format_gitlab_member_access(project, &user.username, access_level).as_str(),
        )?;
        match action {
            Action::Apply => {
                let endpoint = gitlab::api::projects::members::AddProjectMember::builder()
                    .project(project)
                    .user(gitlab_id)
                    .access_level(access_level)
                    .build()
                    .unwrap();
                gitlab::api::ignore(endpoint)
                    .query_async(&self.client)
                    .await
                    .unwrap();
            }
            _ => {}
        }
        Ok(true)
    }

    async fn remove_project_member<'a>(
        &self,
        action: &Action,
        member: &GitLabMember,
        project: &str,
    ) -> Result<bool> {
        debug!(
            "User {} must not be in project {}",
            &member.username, project
        );
        util::print_diff(
            util::format_gitlab_member_access(
                project,
                &member.username,
                util::access_level_from_u64(member.access_level),
            )
            .as_str(),
            &"",
        )?;
        match action {
            Action::Apply => {
                let endpoint = gitlab::api::projects::members::RemoveProjectMember::builder()
                    .project(project)
                    .user(member.id)
                    .build()
                    .unwrap();
                gitlab::api::ignore(endpoint)
                    .query_async(&self.client)
                    .await
                    .unwrap();
            }
            _ => {}
        }
        Ok(true)
    }

    async fn edit_project_member_access_level(
        &self,
        action: &Action,
        user: &User,
        member: &GitLabMember,
        project: &str,
        access_level: AccessLevel,
    ) -> Result<bool> {
        let staff_username = &user.username;
        if user.gitlab_id.is_none() {
            debug!(
                "Skip adding {} to GitLab project: no GitLab user found",
                staff_username
            );
            return Ok(false);
        }
        let gitlab_id = user
            .gitlab_id
            .with_context(|| format!("Failed to unwrap GitLab user for {}", staff_username))?;

        debug!(
            "Adding user {} to GitLab project '{}'",
            user.username, project
        );
        util::print_diff(
            util::format_gitlab_member_access(
                project,
                &user.username,
                util::access_level_from_u64(member.access_level),
            )
            .as_str(),
            util::format_gitlab_member_access(project, &user.username, access_level).as_str(),
        )?;
        match action {
            Action::Apply => {
                let endpoint = gitlab::api::projects::members::EditProjectMember::builder()
                    .project(project)
                    .user(gitlab_id)
                    .access_level(access_level)
                    .build()
                    .unwrap();
                gitlab::api::ignore(endpoint)
                    .query_async(&self.client)
                    .await?;
            }
            _ => {}
        }
        Ok(true)
    }

    async fn edit_project_member_max_access_level(
        &self,
        action: &Action,
        user: &User,
        member: &GitLabMember,
        project: &str,
        max_access_level: AccessLevel,
    ) -> Result<bool> {
        let access_level = util::access_level_from_u64(member.access_level);
        if max_access_level.as_u64() >= access_level.as_u64() {
            debug!(
                "User {} has access_level {} which is below max access_level {} in project {}",
                user.username,
                access_level.as_str(),
                max_access_level.as_str(),
                project,
            );
            return Ok(false);
        }

        self.edit_project_member_access_level(action, user, member, project, max_access_level)
            .await
    }

    async fn apply_project_settings(
        &self,
        action: &Action,
        project: &GroupProjects,
    ) -> Result<bool> {
        let expected_request_access_enabled = false;
        let expected_snippets_access_level = ProjectFeatureAccessLevel::Disabled;

        if project.request_access_enabled == expected_request_access_enabled
            && project.snippets_access_level == expected_snippets_access_level
        {
            return Ok(false);
        }

        debug!("edit project settings for {}", project.name_with_namespace);
        util::print_diff(
            util::format_gitlab_project_settings(
                &project.path_with_namespace,
                project.request_access_enabled,
                project.snippets_access_level,
            )
            .as_str(),
            util::format_gitlab_project_settings(
                &project.path_with_namespace,
                expected_request_access_enabled,
                expected_snippets_access_level,
            )
            .as_str(),
        )?;
        match action {
            Action::Apply => {
                let endpoint = gitlab::api::projects::EditProject::builder()
                    .project(project.id)
                    .request_access_enabled(expected_request_access_enabled)
                    .snippets_access_level(expected_snippets_access_level.as_gitlab_type())
                    .build()
                    .unwrap();
                gitlab::api::ignore(endpoint)
                    .query_async(&self.client)
                    .await?;
            }
            _ => {}
        }
        Ok(true)
    }
}

fn is_archlinux_bot(member: &GitLabMember) -> bool {
    if member.username.eq(GITLAB_OWNER) {
        return true;
    }
    if member.username.eq(GITLAB_BOT) {
        return true;
    }
    // TODO: find a better way for project token users
    // This is not nicely maintainable nor safe to do by regex
    if vec![
        "project_10185_bot2".to_string(),
        "project_19591_bot".to_string(),
    ]
    .contains(&member.username)
    {
        return true;
    }
    false
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
    debug!("protecting tag * for project {}", project.name);
    let endpoint = gitlab::api::projects::protected_tags::ProtectTag::builder()
        .project(project.id)
        .name(tag)
        .create_access_level(gitlab::api::common::ProtectedAccessLevel::Developer)
        .build()
        .unwrap();
    let result: ProtectedTag = endpoint.query(client)?;
    Ok(result)
}
