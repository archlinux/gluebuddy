use std::collections::{HashMap, HashSet};

use crate::components::gitlab::types::Pkgbase;

pub type Username = String;
pub type Group = String;

#[derive(Eq, PartialEq, Debug)]
pub struct User {
    pub username: Username,
    pub gitlab_id: Option<u64>,
    pub groups: HashSet<Group>,
}

#[derive(Eq, PartialEq, Debug)]
pub struct GitLabBot {
    pub username: Username,
    pub gitlab_id: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum PackageMaintainerRole {
    Core,
    JuniorCore,
    Regular,
    Junior,
}

impl PackageMaintainerRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "Core Package Maintainers",
            Self::JuniorCore => "Junior Core Package Maintainers",
            Self::Regular => "Package Maintainers",
            Self::Junior => "Junior Package Maintainers",
        }
    }

    pub fn to_path(self) -> String {
        self.as_str().to_ascii_lowercase().replace(' ', "-")
    }
}

#[derive(Debug, Clone, Copy)]
pub enum WikiMaintainerRole {
    Admin,
    Maintainer,
}

impl WikiMaintainerRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "Admins",
            Self::Maintainer => "Maintainers",
        }
    }

    pub fn to_path(self) -> String {
        self.as_str().to_ascii_lowercase().replace(' ', "-")
    }
}

impl User {
    pub fn new(username: String) -> User {
        User {
            username,
            gitlab_id: None,
            groups: HashSet::new(),
        }
    }

    pub fn is_staff(&self) -> bool {
        self.groups
            .iter()
            .any(|group| group.starts_with("/Arch Linux Staff/"))
    }

    pub fn is_external_contributor(&self) -> bool {
        self.groups
            .iter()
            .any(|group| group.starts_with("/External Contributors"))
    }

    pub fn is_devops(&self) -> bool {
        self.groups
            .iter()
            .any(|group| group.starts_with("/Arch Linux Staff/DevOps"))
    }

    pub fn is_package_maintainer(&self) -> bool {
        self.groups
            .iter()
            .any(|group| group.starts_with("/Arch Linux Staff/Package Maintainer Team/"))
    }

    pub fn is_bug_wrangler(&self) -> bool {
        self.groups
            .iter()
            .any(|group| group.starts_with("/Arch Linux Staff/Bug Wranglers"))
    }

    pub fn has_package_maintainer_role(&self, role: PackageMaintainerRole) -> bool {
        let expected = format!(
            "/Arch Linux Staff/Package Maintainer Team/{}",
            role.as_str()
        );
        self.groups.iter().any(|group| group.starts_with(&expected))
    }

    pub fn has_wiki_maintainer_role(&self, role: WikiMaintainerRole) -> bool {
        let expected = format!("/Arch Linux Staff/Wiki/{}", role.as_str());
        self.groups.iter().any(|group| group.starts_with(&expected))
    }
}

#[derive(Default)]
pub struct State {
    pub users: HashMap<Username, User>,
    pub gitlab_bots: HashMap<u64, GitLabBot>,
    pub pkgbases: HashSet<Pkgbase>,
}

impl State {
    pub fn staff(&self) -> Vec<&User> {
        self.users.values().filter(|user| user.is_staff()).collect()
    }

    pub fn staff_with_externals(&self) -> Vec<&User> {
        self.users
            .values()
            .filter(|user| user.is_staff() || user.is_external_contributor())
            .collect()
    }

    pub fn devops(&self) -> Vec<&User> {
        self.users
            .values()
            .filter(|user| user.is_devops())
            .collect()
    }

    pub fn package_maintainers(&self) -> Vec<&User> {
        self.users
            .values()
            .filter(|user| user.is_package_maintainer())
            .collect()
    }

    pub fn package_maintainers_by_role(&self, role: PackageMaintainerRole) -> Vec<&User> {
        self.users
            .values()
            .filter(|user| user.has_package_maintainer_role(role))
            .collect()
    }

    pub fn bug_wranglers(&self) -> Vec<&User> {
        self.users
            .values()
            .filter(|user| user.is_bug_wrangler())
            .collect()
    }

    pub fn wiki_maintainers_by_role(&self, role: WikiMaintainerRole) -> Vec<&User> {
        self.users
            .values()
            .filter(|user| user.has_wiki_maintainer_role(role))
            .collect()
    }

    pub fn user_from_gitlab_id(&self, gitlab_id: u64) -> Option<&User> {
        self.users.values().find(|user| {
            user.gitlab_id
                .map(|id| id.eq(&gitlab_id))
                .unwrap_or_else(|| false)
        })
    }

    pub fn staff_from_gitlab_id(&self, gitlab_id: u64) -> Option<&User> {
        self.staff().into_iter().find(|user| {
            user.gitlab_id
                .map(|id| id.eq(&gitlab_id))
                .unwrap_or_else(|| false)
        })
    }

    pub fn staff_with_externals_from_gitlab_id(&self, gitlab_id: u64) -> Option<&User> {
        self.staff_with_externals().into_iter().find(|user| {
            user.gitlab_id
                .map(|id| id.eq(&gitlab_id))
                .unwrap_or_else(|| false)
        })
    }

    pub fn devops_from_gitlab_id(&self, gitlab_id: u64) -> Option<&User> {
        self.devops().into_iter().find(|user| {
            user.gitlab_id
                .map(|id| id.eq(&gitlab_id))
                .unwrap_or_else(|| false)
        })
    }

    pub fn package_maintainer_from_gitlab_id(&self, gitlab_id: u64) -> Option<&User> {
        self.package_maintainers().into_iter().find(|user| {
            user.gitlab_id
                .map(|id| id.eq(&gitlab_id))
                .unwrap_or_else(|| false)
        })
    }

    pub fn package_maintainer_from_gitlab_id_and_role(
        &self,
        gitlab_id: u64,
        role: PackageMaintainerRole,
    ) -> Option<&User> {
        self.package_maintainers_by_role(role)
            .into_iter()
            .find(|user| {
                user.gitlab_id
                    .map(|id| id.eq(&gitlab_id))
                    .unwrap_or_else(|| false)
            })
    }

    pub fn wiki_maintainer_from_gitlab_id_and_role(
        &self,
        gitlab_id: u64,
        role: WikiMaintainerRole,
    ) -> Option<&User> {
        self.wiki_maintainers_by_role(role)
            .into_iter()
            .find(|user| {
                user.gitlab_id
                    .map(|id| id.eq(&gitlab_id))
                    .unwrap_or_else(|| false)
            })
    }

    pub fn bug_wrangler_from_gitlab_id(&self, gitlab_id: u64) -> Option<&User> {
        self.bug_wranglers().into_iter().find(|user| {
            user.gitlab_id
                .map(|id| id.eq(&gitlab_id))
                .unwrap_or_else(|| false)
        })
    }

    pub fn is_gitlab_bot(&self, gitlab_uid: u64) -> bool {
        self.gitlab_bots.contains_key(&gitlab_uid)
    }
}
