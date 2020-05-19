use keycloak::{KeycloakAdmin, KeycloakAdminToken};

use reqwest::Client;
use anyhow::{Result, bail};

use std::env;
use std::vec::Vec;
use keycloak::types::GroupRepresentation;
use std::borrow::Cow;

// TODO: error handling for all unwrap shizzle
async fn run() -> Result<()> {
    let username = &env::var("GLUEBUDDY_KEYCLOAK_USERNAME").or_else(
        |_| bail!("Missing GLUEBUDDY_KEYCLOAK_USERNAME env var")
    )?;
    let password = &env::var("GLUEBUDDY_KEYCLOAK_PASSWORD").or_else(
        |_| bail!("Missing GLUEBUDDY_KEYCLOAK_PASSWORD env var")
    )?;
    let realm = &env::var("GLUEBUDDY_KEYCLOAK_REALM").or_else(
        |_| bail!("Missing GLUEBUDDY_KEYCLOAK_REALM env var")
    )?;
    let url = &env::var("GLUEBUDDY_KEYCLOAK_URL").or_else(
        |_| bail!("Missing GLUEBUDDY_KEYCLOAK_URL env var")
    )?;

    let client = Client::new();
    let admin_token = KeycloakAdminToken::acquire(url, username, password, &client).await?;

    let admin = KeycloakAdmin::new(url, admin_token, client);

    let users = admin
        .users_get(realm, None, None, None, None, None, None, None, None)
        .await?;

    for user in users {
        // println!("user: {:?}", user);
    }

    let actions = admin.authentication_required_actions_get(realm).await?;
    for action in actions {
        // println!("action: {:?}", action);
    }

    let groups_for_2fa = vec!["Arch Linux Staff"];

    let groups = admin.groups_get(realm, None, None, None, None).await?;
    let groups = groups.iter().filter(|group| {
        groups_for_2fa.contains(&group.name.as_ref().unwrap().as_ref())
    }).collect::<Vec<_>>();

    for group in groups {
        println!("group: {:?}", group);

        let sub_groups = group.sub_groups.as_ref().unwrap();
        for sub_group in sub_groups {
            println!("-> sub group: {:?}", sub_group);

            // TODO: flatten users and remove duplicates who are in multiple groups
            let members = admin.groups_members_get(realm, sub_group.id.as_ref().unwrap(), None, None, None).await?;
            for member in members {
                println!("-> member: {:?}", member.username.unwrap());
                println!("   required actions: {:?}", member.required_actions);

                // Skip all users that already have a require action to configure TOTP
                if let Some(required_actions) = member.required_actions {
                    if required_actions.contains(&"CONFIGURE_TOTP".into()) {
                        continue;
                    }
                }

                let credentials = admin.users_credentials_get(realm, member.id.as_ref().unwrap().as_ref()).await?;
                println!("-> credentials: {:?}", credentials);

                // TODO: if user has totp in credentials: [] -> skip

                // add docs -> make a second loop and remove require user action for TOTP in case the credentials already have totp, this is required as get->check->put is not race condition free and a user can setup totp in between get->put

                // to reduce window of opportunity, we do an additional get->set->put per user inside a lopp
                let mut member = admin.user_get(realm, &member.id.as_ref().unwrap()).await?;
                member.required_actions = match member.required_actions {
                    None => Some(vec!["CONFIGURE_TOTP".into()]),
                    Some(mut required_actions) => {
                        let totp = "CONFIGURE_TOTP".into();
                        if !required_actions.contains(&totp) {
                            required_actions.push(totp);
                        }
                        Some(required_actions)
                    },
                };
                println!("set required actions: {:?}", member.required_actions);
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    println!("Starting gluebuddy...");
    if let Err(err) = run().await {
        eprintln!("Error: {:?}", err);
        for cause in err.chain() {
            eprintln!("Because: {:?}", cause)
        }
        std::process::exit(1)
    }
}
