# gluebuddy

A secure helper daemon that watches several aspects of the Arch Linux infractructure and makes sure that certain conditions are met.

It glues Arch together. :)

## Usage

Gluebuddy requires the following environment variables to be set:

* GLUEBUDDY_GITLAB_TOKEN - Gitlab bot personal access token
* GLUEBUDDY_GITLAB_BOT_USERS - Optionally set gitlab bot users separated with commas
* GLUEBUDDY_KEYCLOAK_USERNAME - keycloak admin username
* GLUEBUDDY_KEYCLOAK_PASSWORD - keycloak admin password
* GLUEBUDDY_KEYCLOAK_REALM - Keycloak realm
* GLUEBUDDY_KEYCLOAK_URL - Keycloak base url (without trailing /)

## Service account Keycloak

To not use the admin user for obtaining the users/groups a service account can be used which needs to be created in Keycloak.

Create a new client, go to `Clients` and click `Create`:
* enter a client ID 
* make sure client protocl is set to `openid-connect`

In the client settings configure:
* Set Access Type to `Confidential`
* Set `Service Accounts Enabled` to `On`
* Specify a `redirect_uri` even though it is not required
* Click `Save` to save the changes

Go to the `Service Account Roles` tab, select `realm-management` in the `Client roles` dropdown and add:
* query-groups
* view-users

This allows the service account to view users and groups we need in gluebuddy, the username is the `client ID` and the password is the client secret which can be found in the `Credentials` tab.
