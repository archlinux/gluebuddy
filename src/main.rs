extern crate anyhow;

use args::*;
mod args;

use structopt::StructOpt;

use keycloak::{KeycloakAdmin, KeycloakAdminToken};

use reqwest::Client;
use anyhow::{Result, bail};

use std::env;
use std::vec::Vec;
use keycloak::types::{UserRepresentation, CredentialRepresentation};
use futures::future::try_join_all;

use log::{debug, info, error};
use env_logger::Env;

async fn get_user_credentials<'a>(admin: &'a KeycloakAdmin<'a>, realm: &str, member: UserRepresentation<'a>) -> Result<(UserRepresentation<'a>, Vec<CredentialRepresentation<'a>>)> {
    let credentials = admin.users_credentials_get(realm, member.id.as_ref().unwrap().as_ref()).await?;
    Ok((member, credentials))
}

// TODO: error handling for all unwrap shizzle
async fn run(args: Args) -> Result<()> {
    match args.subcommand {
        Some(SubCommand::Completions(completions)) => args::gen_completions(&completions)?,
        _ => run_keycloak(args).await?,
    }
    Ok(())
}

async fn run_keycloak(args: Args) -> Result<()> {
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

    info!("acquire API token for keycloak {} using realm {}", url, realm);
    let admin_token = KeycloakAdminToken::acquire(url, username, password, &client).await?;

    let admin = KeycloakAdmin::new(url, admin_token, client);

    let groups_for_2fa = vec!["Arch Linux Staff"];

    let groups = admin.groups_get(realm, None, None, None, None).await?;
    let groups = groups.iter().filter(|group| {
        groups_for_2fa.contains(&group.name.as_ref().unwrap().as_ref())
    }).collect::<Vec<_>>();

    let groups_members = groups.into_iter().flat_map(|group| {
        let group_name = group.name.as_ref().unwrap().as_ref();
        debug!("processing group: {}", group_name);

        group.sub_groups.as_ref().unwrap().iter().map(|sub_group| {
            info!("collecting members of sub group: {}", sub_group.name.as_ref().unwrap());
            Box::pin(admin.groups_members_get(realm, sub_group.id.as_ref().unwrap(), None, None, None))
        })
    });

    // TODO: remove duplicates who are in multiple groups
    let f = try_join_all(groups_members).await?;
    let members = f.into_iter().flatten().filter(|member| {
        let username = member.username.as_ref().unwrap();
        // Skip all users that already have a require action to configure TOTP
        if let Some(required_actions) = &member.required_actions {
            if required_actions.contains(&"CONFIGURE_TOTP".into()) {
                debug!("CONFIGURE_TOTP present in required actions, skipping user: {}", username);
                return false;
            }
        }
        debug!("CONFIGURE_TOTP not present in required actions, proceeding with user: {}", username);
        true
    }).collect::<Vec<_>>();

    info!("collected {} users whose credentials need to be checked", members.len());

    let users_credentials = try_join_all(members.into_iter().map(|member| get_user_credentials(&admin, realm, member))).await?;
    for (member, credentials) in users_credentials {
        let username = member.username.as_ref().unwrap();
        let credential_types = credentials.iter().map(|credential| credential.type_.as_ref().unwrap().as_ref()).collect::<Vec<_>>();
        let required_actions = member.required_actions.as_ref().map(|actions| actions.into_iter().map(|s| s.as_ref()).collect::<Vec<_>>()).unwrap_or(vec![].into());

        debug!("user {} configured credentials: {:?}, required_actions: {:?}", username, credential_types, required_actions);

        let has_otp = credentials.into_iter().any(|credential| credential.type_.as_ref().map(|type_| type_.eq("otp")).unwrap_or(false));
        if has_otp {
            debug!("otp present in credentials, skipping user: {}", username);
            continue;
        }

        info!("enforce required action CONFIGURE_TOTP for user: {}", username);

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
        // TODO: put back user in non dry mode
    }


    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::from_args();

    let logging = if args.verbose { "debug" } else { "info" };

    env_logger::init_from_env(Env::default()
        .default_filter_or(logging));

    if let Err(err) = run(args).await {
        error!("Error: {:?}", err);
        for cause in err.chain() {
            error!("Caused by: {:?}", cause)
        }
        std::process::exit(1)
    }
}
