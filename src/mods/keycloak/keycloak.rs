//! This module defines keycloak related actions and enforcements.
//!
//! ## Features
//!
//! - enforce multi-factor authentication for all staff members

use crate::args::Action;

use keycloak::types::{CredentialRepresentation, GroupRepresentation, UserRepresentation};
use keycloak::{KeycloakAdmin, KeycloakAdminToken};
use reqwest::Client;

use futures::future::try_join_all;

use anyhow::{Context, Result};
use log::{debug, info};

use std::env;

use crate::state::State;

pub struct Keycloak<'a> {
    admin: KeycloakAdmin<'a>,
    realm: String,
}

impl Keycloak<'_> {
    pub async fn new<'a>() -> Result<Keycloak<'a>> {
        let username = &env::var("GLUEBUDDY_KEYCLOAK_USERNAME")
            .context("Missing env var GLUEBUDDY_KEYCLOAK_USERNAME")?;
        let password = &env::var("GLUEBUDDY_KEYCLOAK_PASSWORD")
            .context("Missing env var GLUEBUDDY_KEYCLOAK_PASSWORD")?;
        let realm = &env::var("GLUEBUDDY_KEYCLOAK_REALM")
            .context("Missing GLUEBUDDY_KEYCLOAK_REALM env var")?;
        let url = &env::var("GLUEBUDDY_KEYCLOAK_URL")
            .context("Missing GLUEBUDDY_KEYCLOAK_URL env var")?;

        let client = Client::new();

        info!(
            "acquire API token for keycloak {} using realm {}",
            url, realm
        );
        let token = KeycloakAdminToken::acquire(url, username, password, &client).await?;
        let admin = KeycloakAdmin::new(url, token, client);

        Ok(Keycloak {
            admin,
            realm: realm.to_string(),
        })
    }

    pub async fn gather<'a>(&'a self, state: &mut State<'a>) -> Result<()> {
        let root_groups = vec!["Arch Linux Staff", "External Contributors"];

        let all_groups = self
            .admin
            .realm_groups_get(&self.realm, None, None, None, None)
            .await?;
        let groups = all_groups
            .iter()
            .filter(|group| root_groups.contains(&group.name.as_ref().unwrap().as_ref()))
            .collect::<Vec<_>>();

        let groups_members = groups.into_iter().flat_map(|group| {
            let group_name = group.name.as_ref().unwrap().as_ref();
            debug!(
                "collect members of group {} via path {}",
                group_name,
                group.path.as_ref().unwrap()
            );
            vec![Box::pin(get_group_members(
                &self.admin,
                &self.realm,
                group.clone(),
            ))]
            .into_iter()
            .chain(group.sub_groups.as_ref().unwrap().iter().map(|sub_group| {
                debug!(
                    "collect members of sub group {}",
                    sub_group.name.as_ref().unwrap(),
                );
                Box::pin(get_group_members(
                    &self.admin,
                    &self.realm,
                    sub_group.clone(),
                ))
            }))
        });

        let group_members = try_join_all(groups_members).await?;

        // TODO: avoid duplicates in staff when multiple groups match
        for (group, users) in group_members {
            for user in users {
                let group_name = group.name.as_ref().unwrap().as_ref();
                println!(
                    "group {} user {}",
                    group_name,
                    user.username.as_ref().unwrap()
                );
                match group_name.as_ref() {
                    "DevOps" => {
                        state.staff.push(user.clone());
                        state.devops.push(user.clone());
                    }
                    "Developers" => {
                        state.staff.push(user.clone());
                        state.developers.push(user.clone());
                    }
                    "Trusted Users" => {
                        state.staff.push(user.clone());
                        state.trusted_users.push(user.clone());
                    }
                    "Security Team" => {
                        // TODO: do not add reporters
                        state.staff.push(user.clone());
                        state.security_team.push(user.clone());
                    }
                    "External Contributors" => {
                        state.external_contributors.push(user.clone());
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    pub async fn run<'a>(&self, state: &State<'a>, action: Action) -> Result<()> {
        Ok(())
    }
}

async fn get_group_members<'a>(
    admin: &'a KeycloakAdmin<'a>,
    realm: &'a str,
    group: GroupRepresentation<'a>,
) -> Result<(GroupRepresentation<'a>, Vec<UserRepresentation<'a>>)> {
    let users = admin
        .realm_groups_with_id_members_get(
            realm,
            group.id.as_ref().unwrap().as_ref(),
            None,
            None,
            None,
        )
        .await?;
    Ok((group, users))
}

async fn users_credentials_get<'a>(
    admin: &'a KeycloakAdmin<'a>,
    realm: &'a str,
    member: UserRepresentation<'a>,
) -> Result<(UserRepresentation<'a>, Vec<CredentialRepresentation<'a>>)> {
    let credentials = admin
        .realm_users_with_id_credentials_get(realm, member.id.as_ref().unwrap().as_ref())
        .await?;
    Ok((member, credentials))
}

// add docs -> make a second loop and remove require user action for TOTP in case the credentials already have totp, this is required as get->check->put is not race condition free and a user can setup totp in between get->put
// to reduce window of opportunity, we do an additional get->set->put per user inside a lopp
async fn users_required_actions_add<'a>(
    admin: &'a KeycloakAdmin<'a>,
    realm: &str,
    member: UserRepresentation<'a>,
) -> Result<()> {
    let mut member = admin
        .realm_users_with_id_get(realm, &member.id.as_ref().unwrap())
        .await?;
    member.required_actions = match member.required_actions {
        None => Some(vec!["CONFIGURE_TOTP".into()]),
        Some(mut required_actions) => {
            let totp = "CONFIGURE_TOTP".into();
            if !required_actions.contains(&totp) {
                required_actions.push(totp);
            }
            Some(required_actions)
        }
    };
    Ok(())
}
