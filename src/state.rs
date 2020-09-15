use keycloak::types::UserRepresentation;

pub struct State<'a> {
    pub staff: Vec<UserRepresentation<'a>>,
    pub developers: Vec<UserRepresentation<'a>>,
    pub trusted_users: Vec<UserRepresentation<'a>>,
    pub devops: Vec<UserRepresentation<'a>>,
    pub security_team: Vec<UserRepresentation<'a>>,
    pub external_contributors: Vec<UserRepresentation<'a>>,
}

impl State<'_> {
    pub fn new<'a>() -> State<'a> {
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
            .map(|e| e.username.as_ref().unwrap())
            .any(|e| e.eq(username))
    }
}
