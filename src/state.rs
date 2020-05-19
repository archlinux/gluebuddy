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

    pub fn is_devops(&self) -> bool {
        self.groups
            .iter()
            .any(|group| group.starts_with("/Arch Linux Staff/DevOps"))
    }
}

pub struct State {
    pub users: HashMap<String, User>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            users: HashMap::new(),
        }
    }
}

impl State {
    pub fn user_may_have_gitlab_archlinux_group_access(&self, username: &str) -> bool {
        self.staff().iter().any(|user| user.username.eq(username))
    }

    pub fn staff(&self) -> Vec<&User> {
        self.users.values().filter(|user| user.is_staff()).collect()
    }

    pub fn devops(&self) -> Vec<&User> {
        self.users
            .values()
            .filter(|user| user.is_devops())
            .collect()
    }

    pub fn user_from_gitlab_id(&self, gitlab_id: u64) -> Option<&User> {
        self.users
            .values()
            .filter(|user| {
                user.gitlab_id
                    .map(|id| id.eq(&gitlab_id))
                    .unwrap_or_else(|| false)
            })
            .next()
    }
}
