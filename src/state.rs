use std::collections::{HashMap, HashSet};

#[derive(Eq, PartialEq, Debug)]
pub struct User {
    pub username: String,
    pub gitlab_id: Option<u64>,
    pub groups: HashSet<String>,
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
}

#[derive(Default)]
pub struct State {
    pub users: HashMap<String, User>,
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

    pub fn bug_wrangler_from_gitlab_id(&self, gitlab_id: u64) -> Option<&User> {
        self.bug_wranglers().into_iter().find(|user| {
            user.gitlab_id
                .map(|id| id.eq(&gitlab_id))
                .unwrap_or_else(|| false)
        })
    }
}
