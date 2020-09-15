use serde::Deserialize;

use strum::VariantNames;
use strum_macros::{EnumString, EnumVariantNames, ToString};

#[derive(Debug, Deserialize)]
pub struct Group {
    pub id: u64,
    pub name: String,
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
    /// The variable type query parameter.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Private => "private",
            Self::Enabled => "enabled",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GroupProjects {
    pub id: u64,
    pub name: String,
    pub visibility: ProjectVisibilityLevel,
    pub request_access_enabled: bool,
    pub container_registry_enabled: bool,
    pub snippets_access_level: ProjectFeatureAccessLevel,
}

#[derive(Debug, Deserialize)]
pub struct AccessLevel {
    pub access_level: u64,
    pub access_level_description: String,
}

#[derive(Debug, Deserialize)]
pub struct ProtectedBranch {
    pub id: u64,
    pub name: String,
    pub push_access_levels: Vec<AccessLevel>,
    pub merge_access_levels: Vec<AccessLevel>,
}

#[derive(Debug, Deserialize)]
pub struct ProtectedTag {
    pub name: String,
    pub create_access_levels: Vec<AccessLevel>,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct ProtectedAccess {
    pub name: String,
    pub push_access_level: MyProtectedAccessLevel,
    pub merge_access_level: MyProtectedAccessLevel,
}

#[derive(Debug, Deserialize)]
pub struct GroupMember {
    pub id: u64,
    pub username: String,
    pub name: String,
    pub email: Option<String>,
    pub access_level: u64,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub name: String,
    pub email: Option<String>,
}

