# gluebuddy

A secure helper daemon that watches several aspects of the Arch Linux infractructure and makes sure that certain conditions are met.

It glues Arch together. :)

## Usage

Gluebuddy requires the following environment variables to be set:

* GLUEBUDDY_GITLAB_TOKEN - Gitlab bot personal access token
* GLUEBUDDY_KEYCLOAK_USERNAME - keycloak admin username
* GLUEBUDDY_KEYCLOAK_PASSWORD - keycloak admin password
* GLUEBUDDY_KEYCLOAK_REALM - Keycloak realm
* GLUEBUDDY_KEYCLOAK_URL - Keycloak base url (without trailing /)
