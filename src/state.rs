use std::collections::{HashMap, HashSet};

#[derive(Eq, PartialEq, Debug)]
pub struct User {
    pub username: String,
    pub gitlab_id: Option<u64>,
    pub groups: HashSet<String>,
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
}
