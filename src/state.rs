use keycloak::types::UserRepresentation;

pub struct User {
    pub username: String,
}

impl User {
    pub fn new(username: String) -> User {
        User {
            username,
        }
    }
}

pub struct State {
    pub staff: Vec<User>,
    pub developers: Vec<User>,
    pub trusted_users: Vec<User>,
    pub devops: Vec<User>,
    pub security_team: Vec<User>,
    pub external_contributors: Vec<User>,
}

impl State {
    pub fn new() -> State {
        State {
            staff: Vec::new(),
            developers: Vec::new(),
            trusted_users: Vec::new(),
            devops: Vec::new(),
            security_team: Vec::new(),
            external_contributors: Vec::new(),
        }
    }

    pub fn user_may_have_gitlab_access(&self, username: &str) -> bool {
        self.staff.iter()
            .chain(self.external_contributors.iter())
            .any(|e| e.username.eq(username))
    }
}
