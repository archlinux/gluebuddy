//! This module defines gitlab related actions and enforcements.
//!
//! ## Features
//!
//! - ensure the integrity of the Arch Linux root group
//!   - add staff members with minimal access
//!   - ensure nobody except devops has higher privileges

use crate::args::Action;
use crate::state::{PackageMaintainerRole, State, User};

use crate::components::gitlab::types::*;

use crate::util;

use std::env;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use base64::Engine;
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
const DEFAULT_PACKAGE_MAINTAINER_ACCESS_LEVEL: AccessLevel = AccessLevel::Developer;
const DEVOPS_INFRASTRUCTURE_ACCESS_LEVEL: AccessLevel = AccessLevel::Developer;
const MAX_ACCESS_LEVEL: AccessLevel = AccessLevel::Developer;

const GITLAB_OWNER: &str = "archceo";
const GITLAB_BOT: &str = "archbot";

const MAIN_BRANCH: &str = "main";
const ALL_TAGS_WILDCARD: &str = "*";

const GROUP_ROOT: &str = "archlinux";
const GROUP_PACKAGES: &str = "archlinux/packaging/packages";

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
        self.update_package_maintainer_groups(&action).await?;
        self.update_bug_wranglers_group_members(&action).await?;
        self.update_infrastructure_project_members(&action).await?;
        Ok(())
    }

    async fn update_archlinux_group_recursively(&self, action: &Action) -> Result<()> {
        let endpoint = gitlab::api::groups::Group::builder()
            .group(GROUP_ROOT)
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

                    let label = format!("GitLab '{}' group members", group.full_name);
                    let mut summary = PlanSummary::new(&label);
                    let members = self.get_group_members(&group.full_path).await?;
                    for member in &members {
                        if is_archlinux_bot(member) {
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

                    let label = format!("GitLab '{}' group settings", group.full_path);
                    let mut summary = PlanSummary::new(&label);

                    if self.apply_group_settings(action, &group).await? {
                        summary.change += 1;
                    }

                    println!("{}", summary);
                    println!("{}", util::format_separator());

                    let projects = self.get_group_projects(&group.full_path).await?;
                    for project in projects {
                        let label =
                            format!("GitLab '{}' project settings", project.name_with_namespace);
                        let mut summary = PlanSummary::new(&label);

                        if project.path_with_namespace.starts_with(GROUP_PACKAGES) {
                            match self
                                .apply_package_project_settings(action, &project)
                                .await?
                            {
                                false => {}
                                true => {
                                    summary.change += 1;
                                }
                            }
                        } else {
                            match self.apply_project_settings(action, &project).await? {
                                false => {}
                                true => {
                                    summary.change += 1;
                                }
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
                            if is_archlinux_bot(member) {
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

                        let label =
                            format!("GitLab '{}' protected tags", project.name_with_namespace);
                        let mut summary = PlanSummary::new(&label);

                        let protected_tags = self.get_protected_tags(&project).await?;

                        let protected_tag = protected_tags
                            .iter()
                            .find(|tag| tag.name.eq(ALL_TAGS_WILDCARD));

                        self.protect_tag(
                            action,
                            &mut summary,
                            &project,
                            ALL_TAGS_WILDCARD,
                            MyProtectedAccessLevel::Developer,
                            protected_tag,
                        )
                        .await?;

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

        for staff in state.staff() {
            if let Some(gitlab_id) = staff.gitlab_id {
                if !archlinux_group_members
                    .iter()
                    .map(|e| e.id)
                    .any(|e| e == gitlab_id)
                    && self
                        .add_group_member(
                            action,
                            staff,
                            group,
                            DEFAULT_ARCH_LINUX_GROUP_ACCESS_LEVEL,
                        )
                        .await?
                {
                    summary.add += 1;
                }
            }
        }

        for member in &archlinux_group_members {
            if is_archlinux_bot(member) {
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

        for staff in state.staff() {
            if let Some(gitlab_id) = staff.gitlab_id {
                if !archlinux_group_members
                    .iter()
                    .map(|e| e.id)
                    .any(|e| e == gitlab_id)
                    && self
                        .add_group_member(action, staff, group, DEFAULT_STAFF_GROUP_ACCESS_LEVEL)
                        .await?
                {
                    summary.add += 1;
                }
            }
        }

        for member in &archlinux_group_members {
            if is_archlinux_bot(member) {
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

        let state = self.state.lock().await;
        for staff in state.devops() {
            if let Some(gitlab_id) = staff.gitlab_id {
                if !group_members.iter().map(|e| e.id).any(|e| e == gitlab_id)
                    && self
                        .add_group_member(
                            action,
                            staff,
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
            if is_archlinux_bot(member) {
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

    async fn update_package_maintainer_groups(&self, action: &Action) -> Result<()> {
        self.update_package_maintainer_group_members(action, PackageMaintainerRole::Core)
            .await?;
        self.update_package_maintainer_group_members(action, PackageMaintainerRole::JuniorCore)
            .await?;
        self.update_package_maintainer_group_members(action, PackageMaintainerRole::Regular)
            .await?;
        self.update_package_maintainer_group_members(action, PackageMaintainerRole::Junior)
            .await?;
        Ok(())
    }

    async fn update_package_maintainer_group_members(
        &self,
        action: &Action,
        role: PackageMaintainerRole,
    ) -> Result<()> {
        let summary_label = format!(
            "GitLab 'Arch Linux/Teams/Package Maintainer Team/{}' group members",
            role.as_str()
        );
        let mut summary = PlanSummary::new(&summary_label);
        let team_path = role.to_path();
        let package_maintainer_group =
            format!("archlinux/teams/package-maintainer-team/{team_path}");
        let group_members = self.get_group_members(&package_maintainer_group).await?;

        let state = self.state.lock().await;
        for staff in state.package_maintainers_by_role(role) {
            if let Some(gitlab_id) = staff.gitlab_id {
                if !group_members.iter().map(|e| e.id).any(|e| e == gitlab_id)
                    && self
                        .add_group_member(
                            action,
                            staff,
                            &package_maintainer_group,
                            DEFAULT_PACKAGE_MAINTAINER_ACCESS_LEVEL,
                        )
                        .await?
                {
                    summary.add += 1;
                }
            }
        }

        for member in &group_members {
            if is_archlinux_bot(member) {
                continue;
            }
            match state.package_maintainer_from_gitlab_id_and_role(member.id, role) {
                None => {
                    if self
                        .remove_group_member(action, &state, member, &package_maintainer_group)
                        .await?
                    {
                        summary.destroy += 1;
                    }
                }
                Some(user) => match util::access_level_from_u64(member.access_level) {
                    DEFAULT_PACKAGE_MAINTAINER_ACCESS_LEVEL => {}
                    _ => {
                        if self
                            .edit_group_member_access_level(
                                action,
                                user,
                                member,
                                &package_maintainer_group,
                                DEFAULT_PACKAGE_MAINTAINER_ACCESS_LEVEL,
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

    async fn update_bug_wranglers_group_members(&self, action: &Action) -> Result<()> {
        let group = "archlinux/teams/bug-wranglers";
        let archlinux_group_members = self.get_group_members(group).await?;

        let mut summary = PlanSummary::new("GitLab 'Arch Linux/Teams/Bug Wranglers' group members");
        let state = self.state.lock().await;

        for user in state.bug_wranglers() {
            if let Some(gitlab_id) = user.gitlab_id {
                if !archlinux_group_members
                    .iter()
                    .map(|e| e.id)
                    .any(|e| e == gitlab_id)
                    && self
                        .add_group_member(action, user, group, DEFAULT_STAFF_GROUP_ACCESS_LEVEL)
                        .await?
                {
                    summary.add += 1;
                }
            }
        }

        for member in &archlinux_group_members {
            if is_archlinux_bot(member) {
                continue;
            }
            match state.bug_wrangler_from_gitlab_id(member.id) {
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
            "",
            util::format_gitlab_member_access(group, &user.username, access_level).as_str(),
        )?;
        if let Action::Apply = action {
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
            "",
        )?;
        if let Action::Apply = action {
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
        if let Action::Apply = action {
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
            "",
            util::format_gitlab_member_access(project, &user.username, access_level).as_str(),
        )?;
        if let Action::Apply = action {
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
            "",
        )?;
        if let Action::Apply = action {
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
        if let Action::Apply = action {
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
        let expected_only_allow_merge_if_all_discussions_are_resolved = true;
        let expected_snippets_access_level = ProjectFeatureAccessLevel::Disabled;
        let description = project.description.clone().unwrap_or_default();

        if project.request_access_enabled == expected_request_access_enabled
            && project.only_allow_merge_if_all_discussions_are_resolved
                == expected_only_allow_merge_if_all_discussions_are_resolved
            && project.snippets_access_level == expected_snippets_access_level
        {
            return Ok(false);
        }

        debug!("edit project settings for {}", project.name_with_namespace);
        util::print_diff(
            util::format_gitlab_project_settings(
                &project.path_with_namespace,
                &description,
                project.request_access_enabled,
                project.issues_access_level,
                project.merge_requests_access_level,
                project.merge_method,
                project.only_allow_merge_if_all_discussions_are_resolved,
                project.builds_access_level,
                project.container_registry_access_level,
                project.packages_enabled,
                project.snippets_access_level,
                project.lfs_enabled,
                project.service_desk_enabled,
                project.pages_access_level,
                project.requirements_access_level,
                project.releases_access_level,
                project.environments_access_level,
                project.feature_flags_access_level,
                project.infrastructure_access_level,
                project.monitor_access_level,
            )
            .as_str(),
            util::format_gitlab_project_settings(
                &project.path_with_namespace,
                &description,
                expected_request_access_enabled,
                project.issues_access_level,
                project.merge_requests_access_level,
                project.merge_method,
                expected_only_allow_merge_if_all_discussions_are_resolved,
                project.builds_access_level,
                project.container_registry_access_level,
                project.packages_enabled,
                expected_snippets_access_level,
                project.lfs_enabled,
                project.service_desk_enabled,
                project.pages_access_level,
                project.requirements_access_level,
                project.releases_access_level,
                project.environments_access_level,
                project.feature_flags_access_level,
                project.infrastructure_access_level,
                project.monitor_access_level,
            )
            .as_str(),
        )?;
        if let Action::Apply = action {
            let endpoint = gitlab::api::projects::EditProject::builder()
                .project(project.id)
                .request_access_enabled(expected_request_access_enabled)
                .only_allow_merge_if_all_discussions_are_resolved(
                    expected_only_allow_merge_if_all_discussions_are_resolved,
                )
                .snippets_access_level(expected_snippets_access_level.as_gitlab_type())
                .build()
                .unwrap();
            gitlab::api::ignore(endpoint)
                .query_async(&self.client)
                .await?;
        }
        Ok(true)
    }

    async fn apply_package_project_settings(
        &self,
        action: &Action,
        project: &GroupProjects,
    ) -> Result<bool> {
        let expected_request_access_enabled = false;
        let expected_packages_enabled = false;
        let expected_lfs_enabled = false;
        let expected_service_desk_enabled = false;
        let expected_issues_access_level = ProjectFeatureAccessLevel::Enabled;
        let expected_merge_requests_access_level = ProjectFeatureAccessLevel::Enabled;
        let expected_merge_method = ProjectMergeMethod::FastForward;
        let expected_only_allow_merge_if_all_discussions_are_resolved = true;
        let expected_builds_access_level = ProjectFeatureAccessLevel::Disabled;
        let expected_container_registry_access_level = ProjectFeatureAccessLevel::Disabled;
        let expected_snippets_access_level = ProjectFeatureAccessLevel::Disabled;
        let expected_pages_access_level = ProjectFeatureAccessLevelPublic::Disabled;
        let expected_requirements_access_level = ProjectFeatureAccessLevel::Disabled;
        let expected_releases_access_level = ProjectFeatureAccessLevel::Disabled;
        let expected_environments_access_level = ProjectFeatureAccessLevel::Disabled;
        let expected_feature_flags_access_level = ProjectFeatureAccessLevel::Disabled;
        let expected_infrastructure_access_level = ProjectFeatureAccessLevel::Disabled;
        let expected_monitor_access_level = ProjectFeatureAccessLevel::Disabled;

        let description = project.description.clone().unwrap_or_default();
        let expected_description = self
            .project_description_from_packaging_srcinfo(project)
            .await?;

        if description == expected_description
            && project.request_access_enabled == expected_request_access_enabled
            && project.packages_enabled == expected_packages_enabled
            && project.lfs_enabled == expected_lfs_enabled
            && project.service_desk_enabled == expected_service_desk_enabled
            && project.issues_access_level == expected_issues_access_level
            && project.merge_requests_access_level == expected_merge_requests_access_level
            && project.merge_method == expected_merge_method
            && project.only_allow_merge_if_all_discussions_are_resolved
                == expected_only_allow_merge_if_all_discussions_are_resolved
            && project.builds_access_level == expected_builds_access_level
            && project.container_registry_access_level == expected_container_registry_access_level
            && project.snippets_access_level == expected_snippets_access_level
            && project.pages_access_level == expected_pages_access_level
            && project.requirements_access_level == expected_requirements_access_level
            && project.releases_access_level == expected_releases_access_level
            && project.environments_access_level == expected_environments_access_level
            && project.feature_flags_access_level == expected_feature_flags_access_level
            && project.infrastructure_access_level == expected_infrastructure_access_level
            && project.monitor_access_level == expected_monitor_access_level
        {
            return Ok(false);
        }

        debug!("edit project settings for {}", project.name_with_namespace);
        util::print_diff(
            util::format_gitlab_project_settings(
                &project.path_with_namespace,
                &description,
                project.request_access_enabled,
                project.issues_access_level,
                project.merge_requests_access_level,
                project.merge_method,
                project.only_allow_merge_if_all_discussions_are_resolved,
                project.builds_access_level,
                project.container_registry_access_level,
                project.packages_enabled,
                project.snippets_access_level,
                project.lfs_enabled,
                project.service_desk_enabled,
                project.pages_access_level,
                project.requirements_access_level,
                project.releases_access_level,
                project.environments_access_level,
                project.feature_flags_access_level,
                project.infrastructure_access_level,
                project.monitor_access_level,
            )
            .as_str(),
            util::format_gitlab_project_settings(
                &project.path_with_namespace,
                &expected_description,
                expected_request_access_enabled,
                expected_issues_access_level,
                expected_merge_requests_access_level,
                expected_merge_method,
                expected_only_allow_merge_if_all_discussions_are_resolved,
                expected_builds_access_level,
                expected_container_registry_access_level,
                expected_packages_enabled,
                expected_snippets_access_level,
                expected_lfs_enabled,
                expected_service_desk_enabled,
                expected_pages_access_level,
                expected_requirements_access_level,
                expected_releases_access_level,
                expected_environments_access_level,
                expected_feature_flags_access_level,
                expected_infrastructure_access_level,
                expected_monitor_access_level,
            )
            .as_str(),
        )?;
        if let Action::Apply = action {
            let endpoint = gitlab::api::projects::EditProject::builder()
                .project(project.id)
                .description(expected_description)
                .request_access_enabled(expected_request_access_enabled)
                .packages_enabled(expected_packages_enabled)
                .lfs_enabled(expected_lfs_enabled)
                .service_desk_enabled(expected_service_desk_enabled)
                .issues_access_level(expected_issues_access_level.as_gitlab_type())
                .merge_requests_access_level(expected_merge_requests_access_level.as_gitlab_type())
                .merge_method(expected_merge_method.as_gitlab_type())
                .only_allow_merge_if_all_discussions_are_resolved(
                    expected_only_allow_merge_if_all_discussions_are_resolved,
                )
                .builds_access_level(expected_builds_access_level.as_gitlab_type())
                .container_registry_access_level(
                    expected_container_registry_access_level.as_gitlab_type(),
                )
                .snippets_access_level(expected_snippets_access_level.as_gitlab_type())
                .pages_access_level(expected_pages_access_level.as_gitlab_type())
                .requirements_access_level(expected_requirements_access_level.as_gitlab_type())
                .releases_access_level(expected_releases_access_level.as_gitlab_type())
                .environments_access_level(expected_environments_access_level.as_gitlab_type())
                .feature_flags_access_level(expected_feature_flags_access_level.as_gitlab_type())
                .infrastructure_access_level(expected_infrastructure_access_level.as_gitlab_type())
                .monitor_access_level(expected_monitor_access_level.as_gitlab_type())
                .build()
                .unwrap();
            gitlab::api::ignore(endpoint)
                .query_async(&self.client)
                .await?;
        }
        Ok(true)
    }

    async fn apply_group_settings(&self, action: &Action, group: &Group) -> Result<bool> {
        let expected_request_access_enabled = false;

        if group.request_access_enabled == expected_request_access_enabled {
            return Ok(false);
        }

        debug!("edit group settings for {}", group.full_path);
        util::print_diff(
            util::format_gitlab_group_settings(&group.full_path, group.request_access_enabled)
                .as_str(),
            util::format_gitlab_group_settings(&group.full_path, expected_request_access_enabled)
                .as_str(),
        )?;
        if let Action::Apply = action {
            let endpoint = gitlab::api::groups::EditGroup::builder()
                .group(group.id)
                .request_access_enabled(expected_request_access_enabled)
                .build()
                .unwrap();
            gitlab::api::ignore(endpoint)
                .query_async(&self.client)
                .await?;
        }
        Ok(true)
    }

    async fn get_protected_tags(&self, project: &GroupProjects) -> Result<Vec<ProtectedTag>> {
        let endpoint = gitlab::api::projects::protected_tags::ProtectedTags::builder()
            .project(project.id)
            .build()
            .unwrap();
        let protected_tag: Vec<ProtectedTag> = endpoint.query_async(&self.client).await?;
        Ok(protected_tag)
    }

    async fn get_protected_tag(&self, project: &GroupProjects, tag: &str) -> Result<ProtectedTag> {
        let endpoint = gitlab::api::projects::protected_tags::ProtectedTag::builder()
            .project(project.id)
            .name(tag)
            .build()
            .unwrap();
        let protected_tag: ProtectedTag = endpoint.query_async(&self.client).await?;
        Ok(protected_tag)
    }

    async fn protect_tag(
        &self,
        action: &Action,
        plan: &mut PlanSummary,
        project: &GroupProjects,
        tag: &str,
        allowed_to_create: MyProtectedAccessLevel,
        current_tag: Option<&ProtectedTag>,
    ) -> Result<bool> {
        debug!(
            "protecting tag {} for project {}",
            tag, project.name_with_namespace
        );

        let mut protect = false;
        let mut unprotect = false;

        let access_vec = vec![allowed_to_create.clone()];
        match current_tag {
            Some(current_tag) => {
                let current_levels: Vec<MyProtectedAccessLevel> = current_tag
                    .create_access_levels
                    .iter()
                    .map(|level| level.as_gitlab_type())
                    .collect();

                if current_levels.len() > 1 || !current_levels.contains(&allowed_to_create) {
                    plan.change += 1;
                    protect = true;
                    unprotect = true;

                    util::print_diff(
                        util::format_gitlab_project_protected_tag(
                            &project.path_with_namespace,
                            &current_tag.name,
                            &current_levels,
                        )
                        .as_str(),
                        util::format_gitlab_project_protected_tag(
                            &project.path_with_namespace,
                            tag,
                            &access_vec,
                        )
                        .as_str(),
                    )?;
                }
            }
            None => {
                plan.add += 1;
                protect = true;

                util::print_diff(
                    "",
                    util::format_gitlab_project_protected_tag(
                        &project.path_with_namespace,
                        tag,
                        &access_vec,
                    )
                    .as_str(),
                )?;
            }
        }

        if let Action::Apply = action {
            if protect {
                if unprotect {
                    // TODO: API has to PATCH, remove unprotect if its upstreamed
                    let endpoint = gitlab::api::projects::protected_tags::UnprotectTag::builder()
                        .project(project.id)
                        .name(tag)
                        .build()
                        .unwrap();
                    gitlab::api::ignore(endpoint)
                        .query_async(&self.client)
                        .await?;
                }

                let endpoint = gitlab::api::projects::protected_tags::ProtectTag::builder()
                    .project(project.id)
                    .name(tag)
                    .create_access_level(allowed_to_create.as_gitlab_type())
                    .build()
                    .unwrap();
                gitlab::api::ignore(endpoint)
                    .query_async(&self.client)
                    .await?;
            }
        }
        Ok(true)
    }

    async fn project_description_from_packaging_srcinfo(
        &self,
        project: &GroupProjects,
    ) -> Result<String> {
        let mut description = "".to_string();

        let file_endpoint = gitlab::api::projects::repository::files::File::builder()
            .project(project.id)
            .file_path(".SRCINFO")
            .ref_("main")
            .build()
            .unwrap();
        let srcinfo: Result<File, _> = file_endpoint.query_async(&self.client).await;

        if let Ok(srcinfo) = srcinfo {
            let bytes = base64::engine::general_purpose::STANDARD.decode(&srcinfo.content)?;
            let srcinfo = String::from_utf8(bytes)?;

            let mut pkgnames: Vec<String> = vec![];

            for line in srcinfo.lines() {
                let line = line.trim();
                if line.starts_with("pkgdesc = ") && description.is_empty() {
                    let line = line.replace("pkgdesc = ", "");
                    description = line.clone();
                } else if line.starts_with("pkgname = ") && pkgnames.len() < 16 {
                    let line = line.replace("pkgname = ", "");
                    pkgnames.push(line.clone());
                    if pkgnames.len() == 16 {
                        pkgnames.push("...".to_string())
                    }
                }
            }

            description = format!("{}\n\npackages: {}", description, pkgnames.join(" "));
            description.truncate(2000);
        }

        Ok(description)
    }
}

fn is_archlinux_bot(member: &GitLabMember) -> bool {
    if member.username.eq(GITLAB_OWNER) {
        return true;
    }
    if member.username.eq(GITLAB_BOT) {
        return true;
    }
    let bot_users_list = env::var_os("GLUEBUDDY_GITLAB_BOT_USERS");
    if let Some(list) = bot_users_list {
        return list
            .into_string()
            .unwrap()
            .split(',')
            .any(|bot_name| member.username.eq(bot_name));
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
    gitlab::api::ignore(endpoint).query(client)?;
    Ok(())
}

fn unprotect_tag(client: &Gitlab, project: &GroupProjects, tag: &str) -> Result<()> {
    let endpoint = gitlab::api::projects::protected_tags::UnprotectTag::builder()
        .project(project.id)
        .name(tag)
        .build()
        .unwrap();
    gitlab::api::ignore(endpoint).query(client)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serial_test::serial;

    const SOME_KNOWN_BOTS: &str = "project_10185_bot2,project_19591_bot,project_19796_bot,renovate";

    #[rstest]
    #[case(None, GITLAB_OWNER, true)]
    #[case(None, GITLAB_BOT, true)]
    #[case(Some(SOME_KNOWN_BOTS), "renovate", true)]
    #[case(Some(SOME_KNOWN_BOTS), "renovate_kitty", false)]
    #[case(Some(SOME_KNOWN_BOTS), "project_10185_bot2", true)]
    #[case(Some(SOME_KNOWN_BOTS), "project_19591_bot", true)]
    #[case(Some(SOME_KNOWN_BOTS), "project_19796_bot", true)]
    #[case(None, "test_bot_user", false)]
    #[case(Some("another_test_user"), "test_bot_user", false)]
    #[case(Some(SOME_KNOWN_BOTS), "test_bot_user", false)]
    #[serial]
    fn is_archlinux_bot_test(
        #[case] bot_users_env: Option<&str>,
        #[case] username: &str,
        #[case] expected: bool,
    ) {
        match bot_users_env {
            None => env::remove_var("GLUEBUDDY_GITLAB_BOT_USERS"),
            Some(x) => env::set_var("GLUEBUDDY_GITLAB_BOT_USERS", x),
        }
        let member = GitLabMember {
            id: 0,
            username: String::from(username),
            name: String::from(""),
            email: None,
            access_level: 0,
        };
        assert_eq!(is_archlinux_bot(&member), expected);
    }
}
