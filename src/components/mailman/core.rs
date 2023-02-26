//! This module defines mailman related actions and enforcements.
//!
//! ## Features
//!

use crate::args::Action;
use crate::state::{State, User};
use crate::util;

use base64::{Engine as _, engine::general_purpose};

use reqwest::Client;
use reqwest::header;

use futures::future::try_join_all;
use crate::components::gitlab::types::PlanSummary;

use anyhow::{Context, Result};
use log::{debug, info};
use serde_json::json;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use std::env;
use std::sync::Arc;

pub struct Mailman {
    url: String,
    client: Client,
    state: Arc<Mutex<State>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Membership {
    address: String,
    bounce_score: u8,
    delivery_mode: String,
    display_name: String,
    email: String,
    http_etag: String,
    list_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Memberships {
    entries: Option<Vec<Membership>>
}

impl Mailman {
    pub fn new(state: Arc<Mutex<State>>) -> Result<Mailman> {
        let username = &env::var("GLUEBUDDY_MAILMAN_USERNAME")
            .context("Missing env var GLUEBUDDY_MAILMAN_USERNAME")?;
        let password = &env::var("GLUEBUDDY_MAILMAN_PASSWORD")
            .context("Missing env var GLUEBUDDY_MAILMAN_PASSWORD")?;
        let url = &env::var("GLUEBUDDY_MAILMAN_URL")
            .context("Missing GLUEBUDDY_MAILMAN_URL env var")?;

        // https://github.com/seanmonstar/reqwest/issues/1383
        let mut headers = header::HeaderMap::new();
        let base64_secret = general_purpose::STANDARD.encode(format!("{}:{}", username, password));
        let mut auth_value = header::HeaderValue::from_str(&format!("Basic {}", base64_secret))?;
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);

        // TODO: add version
        let client = Client::builder().user_agent("gluebuddy").default_headers(headers).build()?;

        Ok(Mailman {
            url: url.to_string(),
            client,
            state,
        })
    }

    pub fn membership_url(&self, user: &User) -> String {
        format!("{}/3.1/addresses/{}/memberships", &self.url, user.email)
    }

    pub async fn gather(&self) -> Result<()> {
        info!("Gathering Mailman state");
        // http://localhost:8001/3.1/addresses/jelle@archlinux.org/memberships | jq -r '.entries[].list_id'
        let mut state = self.state.lock().await;

        for user in &mut state.users.values_mut() {
            let url = self.membership_url(&user);
            // let json: &serde_json::Value = &self.client.get(url).send().await?.json().await?;
            let memberships: &Memberships = &self.client.get(url).send().await?.json().await?;
            if let Some(entries) = &memberships.entries {
                for entry in entries {
                    user.memberships.insert(entry.list_id.clone());
                }
            }
        }
        Ok(())
    }

    async fn check_membership(&self, action: &Action, mailing_list: &str) -> Result<()> {
        let mut state = self.state.lock().await;
        let label = format!("Mailman Staff mailing list '{}' group members", mailing_list);
        let mut summary = PlanSummary::new(&label);
        for user in &mut state.users.values_mut() {
            if ! user.memberships.contains(mailing_list) {
                summary.add += 1;
            }
        }

        println!("{}", summary);
        println!("{}", util::format_separator());
        Ok(())
    }

    pub async fn run(&self, action: Action) -> Result<()> {
        self.check_membership(&action, "staff.lists.archlinux.org").await?;
        self.check_membership(&action, "arch-dev-public.lists.archlinux.org").await?;
        Ok(())
    }
}
