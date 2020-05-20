use keycloak::{KeycloakAdmin, KeycloakAdminToken};

use reqwest::Client;
use anyhow::{Result, bail};

use std::env;
use std::vec::Vec;
use keycloak::types::{UserRepresentation, CredentialRepresentation};
use futures::future::try_join_all;

async fn get_user_credentials<'a>(admin: &'a KeycloakAdmin<'a>, realm: &str, member: UserRepresentation<'a>) -> Result<(UserRepresentation<'a>, Vec<CredentialRepresentation<'a>>)> {
    let credentials = admin.users_credentials_get(realm, member.id.as_ref().unwrap().as_ref()).await?;
    Ok((member, credentials))
}

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

    let groups_for_2fa = vec!["Arch Linux Staff"];

    let groups = admin.groups_get(realm, None, None, None, None).await?;
    let groups = groups.iter().filter(|group| {
        groups_for_2fa.contains(&group.name.as_ref().unwrap().as_ref())
    }).collect::<Vec<_>>();


    let groups_members = groups.into_iter().flat_map(|group| {
        println!("group: {:?}", group);

        group.sub_groups.as_ref().unwrap().iter().map(|sub_group| {
            println!("-> sub group: {:?}", sub_group);
            Box::pin(admin.groups_members_get(realm, sub_group.id.as_ref().unwrap(), None, None, None))
        })
    });

    // TODO: remove duplicates who are in multiple groups
    let f = try_join_all(groups_members).await?;
    let members = f.into_iter().flatten().filter(|member| {
        // Skip all users that already have a require action to configure TOTP
        if let Some(required_actions) = &member.required_actions {
            return !required_actions.contains(&"CONFIGURE_TOTP".into());
        }
        true
    }).collect::<Vec<_>>();

    println!("members: {}", members.len());

    let users_credentials = try_join_all(members.into_iter().map(|member| get_user_credentials(&admin, realm, member))).await?;
    for (member, credentials) in users_credentials {
        println!("-> member: {:?}", member.username.unwrap());
        //println!("-> member: {:?}", member.username.unwrap());
        //println!("   required actions: {:?}", member.required_actions);

        let has_otp = credentials.iter().any(|credential| credential.type_.as_ref().map(|type_| type_.eq("otp")).unwrap_or(false));
        if has_otp {
            continue;
        }

        println!("-> credentials: {:?}", credentials);

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
        // TODO: put back user in non dry mode
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
