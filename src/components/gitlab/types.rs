use gitlab::api::groups::BranchProtection;
use gitlab::api::projects::{FeatureAccessLevel, FeatureAccessLevelPublic, MergeMethod};
use serde::Deserialize;
use serde_repr::*;
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use strum_macros::EnumString;

use time::OffsetDateTime;

pub type Packager = String;
pub type Pkgbase = String;
pub type PkgbaseMaintainers = HashMap<Pkgbase, Vec<Packager>>;

#[derive(Debug, Deserialize)]
pub struct PlanSummary {
    name: String,
    pub add: u64,
    pub change: u64,
    pub destroy: u64,
}

impl PlanSummary {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            add: 0,
            change: 0,
            destroy: 0,
        }
    }
}

impl Display for PlanSummary {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if 0 == self.add && 0 == self.change && 0 == self.destroy {
            return write!(f, "No changes. {} is up-to-date.", self.name);
        }
        write!(
            f,
            "{} has changed!\nPlan: {} to add, {} to change, {} to destroy.",
            self.name, self.add, self.change, self.destroy
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct Group {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub path: String,
    pub full_path: String,
    pub request_access_enabled: bool,
    pub default_branch_protection: GroupBranchProtection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectVisibilityLevel {
    /// The project is visible to anonymous users.
    Public,
    /// The project is visible to logged in users.
    Internal,
    /// The project is visible only to users with explicit access.
    Private,
}

impl ProjectVisibilityLevel {
    /// The string representation of the visibility level.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Internal => "internal",
            Self::Private => "private",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectFeatureAccessLevel {
    /// The feature is not available at all.
    Disabled,
    /// The features is only available to project members.
    Private,
    /// The feature is available to everyone with access to the project.
    Enabled,
}

impl ProjectFeatureAccessLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Private => "private",
            Self::Enabled => "enabled",
        }
    }

    pub fn as_gitlab_type(self) -> FeatureAccessLevel {
        match self {
            Self::Disabled => FeatureAccessLevel::Disabled,
            Self::Private => FeatureAccessLevel::Private,
            Self::Enabled => FeatureAccessLevel::Enabled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectFeatureAccessLevelPublic {
    /// The feature is not available at all.
    Disabled,
    /// The features is only available to project members.
    Private,
    /// The feature is available to everyone with access to the project.
    Enabled,
    /// The feature is publicly available regardless of project access.
    Public,
}

impl ProjectFeatureAccessLevelPublic {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Private => "private",
            Self::Enabled => "enabled",
            Self::Public => "public",
        }
    }

    pub fn as_gitlab_type(self) -> FeatureAccessLevelPublic {
        match self {
            Self::Disabled => FeatureAccessLevelPublic::Disabled,
            Self::Private => FeatureAccessLevelPublic::Private,
            Self::Enabled => FeatureAccessLevelPublic::Enabled,
            Self::Public => FeatureAccessLevelPublic::Public,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum GroupBranchProtection {
    /// Developers and maintainers may push, force push, and delete branches.
    None = 0,
    /// Developers and maintainers may push branches.
    Partial = 1,
    /// Maintainers may push branches.
    Full = 2,
}

impl GroupBranchProtection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Partial => "partial",
            Self::Full => "full",
        }
    }

    pub fn as_gitlab_type(self) -> BranchProtection {
        match self {
            Self::None => BranchProtection::None,
            Self::Partial => BranchProtection::Partial,
            Self::Full => BranchProtection::Full,
        }
    }
}

/// How merge requests should be merged when using the "Merge" button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectMergeMethod {
    /// Always create a merge commit.
    Merge,
    /// Always create a merge commit, but require that the branch be fast-forward capable.
    RebaseMerge,
    /// Only fast-forward merges are allowed.
    #[serde(rename = "ff")]
    FastForward,
}

impl ProjectMergeMethod {
    /// The variable type query parameter.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::RebaseMerge => "rebase_merge",
            Self::FastForward => "ff",
        }
    }

    pub fn as_gitlab_type(self) -> MergeMethod {
        match self {
            ProjectMergeMethod::Merge => MergeMethod::Merge,
            ProjectMergeMethod::RebaseMerge => MergeMethod::RebaseMerge,
            ProjectMergeMethod::FastForward => MergeMethod::FastForward,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GroupProjects {
    pub id: u64,
    pub archived: bool,
    #[serde(with = "time::serde::iso8601")]
    pub updated_at: OffsetDateTime,
    pub name: String,
    pub name_with_namespace: String,
    pub path: String,
    pub path_with_namespace: String,
    pub description: Option<String>,
    pub visibility: ProjectVisibilityLevel,
    pub request_access_enabled: bool,
    pub packages_enabled: bool,
    pub lfs_enabled: bool,
    pub service_desk_enabled: bool,
    pub issues_access_level: ProjectFeatureAccessLevel,
    pub merge_requests_access_level: ProjectFeatureAccessLevel,
    pub merge_method: ProjectMergeMethod,
    pub only_allow_merge_if_all_discussions_are_resolved: bool,
    pub builds_access_level: ProjectFeatureAccessLevel,
    pub releases_access_level: ProjectFeatureAccessLevel,
    pub container_registry_access_level: ProjectFeatureAccessLevel,
    pub snippets_access_level: ProjectFeatureAccessLevel,
    pub pages_access_level: ProjectFeatureAccessLevelPublic,
    pub requirements_access_level: ProjectFeatureAccessLevel,
    pub environments_access_level: ProjectFeatureAccessLevel,
    pub feature_flags_access_level: ProjectFeatureAccessLevel,
    pub infrastructure_access_level: ProjectFeatureAccessLevel,
    pub monitor_access_level: ProjectFeatureAccessLevel,
}

#[derive(Debug, Deserialize)]
pub struct ProtectedAccessLevel {
    pub access_level: u64,
    pub access_level_description: String,
}

impl ProtectedAccessLevel {
    pub fn as_str(&self) -> &str {
        &self.access_level_description
    }

    pub fn as_gitlab_type(&self) -> MyProtectedAccessLevel {
        match self.access_level {
            60 => MyProtectedAccessLevel::Admin,
            40 => MyProtectedAccessLevel::Maintainer,
            30 => MyProtectedAccessLevel::Developer,
            _ => MyProtectedAccessLevel::NoAccess,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ProtectedBranch {
    pub id: u64,
    pub name: String,
    pub push_access_levels: Vec<ProtectedAccessLevel>,
    pub merge_access_levels: Vec<ProtectedAccessLevel>,
}

#[derive(Debug, Deserialize)]
pub struct ProtectedTag {
    pub name: String,
    pub create_access_levels: Vec<ProtectedAccessLevel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum MyProtectedAccessLevel {
    /// The action is not allowed at all.
    NoAccess,
    /// Developers and maintainers may perform the action.
    Developer,
    /// Maintainers may perform the action.
    Maintainer,
    /// Only administrators may perform the action.
    Admin,
}

impl MyProtectedAccessLevel {
    pub fn as_gitlab_type(&self) -> gitlab::api::common::ProtectedAccessLevel {
        match self {
            Self::Admin => gitlab::api::common::ProtectedAccessLevel::Admin,
            Self::Maintainer => gitlab::api::common::ProtectedAccessLevel::Maintainer,
            Self::Developer => gitlab::api::common::ProtectedAccessLevel::Developer,
            Self::NoAccess => gitlab::api::common::ProtectedAccessLevel::NoAccess,
        }
    }

    /// The variable type query parameter.
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Maintainer => "maintainer",
            Self::Developer => "developer",
            Self::NoAccess => "no_access",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ProtectedAccess {
    pub name: String,
    pub push_access_level: MyProtectedAccessLevel,
    pub merge_access_level: MyProtectedAccessLevel,
}

#[derive(Debug, Deserialize)]
pub struct GitLabMember {
    pub id: u64,
    pub username: String,
    pub name: String,
    pub email: Option<String>,
    pub access_level: u64,
}

#[derive(Debug, Deserialize)]
pub struct GitLabUser {
    pub id: u64,
    pub username: String,
    pub name: String,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct File {
    pub content: String,
}
