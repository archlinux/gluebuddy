//! This module defines keycloak related actions and enforcements.
//!
//! ## Features
//!

use crate::args::Action;

use keycloak::types::{GroupRepresentation, UserRepresentation};
use keycloak::{KeycloakAdmin, KeycloakAdminToken, KeycloakError};
use reqwest::Client;

use futures::future::try_join_all;

use anyhow::{Context, Result};
use log::{debug, info};
use serde_json::json;
use tokio::sync::Mutex;

use std::env;
use std::sync::Arc;

use crate::state::State;
use crate::state::User;

pub struct Keycloak {
    admin: KeycloakAdmin,
    realm: String,
    state: Arc<Mutex<State>>,
}

impl Keycloak {
    pub async fn new(state: Arc<Mutex<State>>) -> Result<Keycloak> {
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

        let token = Self::acquire_custom_realm(
            url,
            realm,
            username,
            password,
            "client_credentials",
            &client,
        )
        .await?;
        let admin = KeycloakAdmin::new(url, token, client);

        Ok(Keycloak {
            admin,
            realm: realm.to_string(),
            state,
        })
    }

    async fn acquire_custom_realm(
        url: &str,
        realm: &str,
        client_id: &str,
        client_secret: &str,
        grant_type: &str,
        client: &Client,
    ) -> Result<KeycloakAdminToken, KeycloakError> {
        let response = client
            .post(&format!(
                "{}/realms/{}/protocol/openid-connect/token",
                url, realm
            ))
            .form(&json!({
                "client_id": client_id,
                "client_secret": client_secret,
                "grant_type": grant_type
            }))
            .send()
            .await?;

        Ok(Self::error_check(response).await?.json().await?)
    }

    async fn error_check(response: reqwest::Response) -> Result<reqwest::Response, KeycloakError> {
        if !response.status().is_success() {
            let status = response.status().into();
            let text = response.text().await?;
            return Err(KeycloakError::HttpFailure {
                status,
                body: serde_json::from_str(&text).ok(),
                text,
            });
        }

        Ok(response)
    }

    pub async fn gather(&self) -> Result<()> {
        info!("Gathering Keycloak state");
        let root_groups = ["Arch Linux Staff", "External Contributors"];

        let all_groups = self
            .admin
            .realm_groups_get(&self.realm, None, None, None, None, None, None)
            .await?;

        let groups_members = all_groups
            .iter()
            .filter(|group| root_groups.contains(&group.name.as_ref().unwrap().as_ref()))
            .flat_map(|group| {
                let group_name = group.name.as_ref().unwrap();
                info!(
                    "collect members of group {} via {}",
                    group_name,
                    group.path.as_ref().unwrap()
                );
                vec![Box::pin(self.get_group_members(group.clone()))]
                    .into_iter()
                    .chain(group.sub_groups.as_ref().unwrap().iter().map(|sub_group| {
                        info!(
                            "collect members of sub group {} via {}",
                            sub_group.name.as_ref().unwrap(),
                            sub_group.path.as_ref().unwrap()
                        );
                        Box::pin(self.get_group_members(sub_group.clone()))
                    }))
                    .chain(
                        group
                            .sub_groups
                            .as_ref()
                            .unwrap()
                            .iter()
                            .flat_map(|sub_group| sub_group.sub_groups.as_ref().unwrap())
                            .map(|sub_group| {
                                info!(
                                    "collect members of sub group {} via {}",
                                    sub_group.name.as_ref().unwrap(),
                                    sub_group.path.as_ref().unwrap(),
                                );
                                Box::pin(self.get_group_members(sub_group.clone()))
                            }),
                    )
            });

        let group_members = try_join_all(groups_members).await?;
        let mut state = self.state.lock().await;

        for (group, users) in group_members {
            for user in users {
                let group_name = group.name.as_ref().unwrap();
                let path = group.path.as_ref().unwrap();
                debug!(
                    "group {} via {} user {}",
                    group_name,
                    path,
                    user.username.as_ref().unwrap()
                );

                let state_user = state
                    .users
                    .entry(user.username.as_ref().unwrap().to_string())
                    .or_insert_with_key(|key| {
                        let arch_email = if let Some(attributes) = user.attributes {
                            attributes.get("arch_email").unwrap()[0]
                                .as_str()
                                .unwrap()
                                .to_string()
                        } else {
                            "".to_string()
                        };
                        User::new(
                            key.clone(),
                            user.email.as_ref().unwrap().to_string(),
                            arch_email,
                        )
                    });
                state_user.groups.insert(path.to_string());
            }
        }

        Ok(())
    }

    pub async fn run(&self, _action: Action) -> Result<()> {
        Ok(())
    }

    async fn get_group_members(
        &self,
        group: GroupRepresentation,
    ) -> Result<(GroupRepresentation, Vec<UserRepresentation>)> {
        let users = self
            .admin
            .realm_groups_with_id_members_get(
                &self.realm,
                group.id.as_ref().unwrap().as_ref(),
                None,
                None,
                None,
            )
            .await?;
        Ok((group, users))
    }
}
