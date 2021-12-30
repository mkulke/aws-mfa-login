# AWS MFA Login

## Why

- We want to enforce MFA for users in an AWS account.
- Users should assume roles with privileges requried for specific workloads, instead of having static user/group privileges.
- Temporary credentials should be cached (boto3 does this, but not all aws sdks do. notably the js sdk and hence CDK acquire new tokens for every action, which in combination with MFA is pretty nasty).

## How

We have an iam Role which has certain policies attached (`AdministratorAccess` in our case). This role can only be assumed if the user has been authenticated with a MFA device (`"Bool": { "aws:MultiFactorAuthPresent": true }`).

There is an iam Group in which we can put users. The users are allow to assume the Role above, the constraints of the to-be-assumed role (MFA required) still have to be considered.

A iam Policy which is assigned to the Group, to allow users in the group to manage their MFA user settings, however they cannot remove/disable a MFA device (and thus lock themselves out).

In `./cfn` there is a CloudFormation template to create the above resources, based on a template from [Mattias Severson](https://blog.jayway.com/2017/11/22/aws-cli-mfa).

The cli tool `aws-mfa-login` is invoked with a `--token`, which is retrieved from an MFA device that has been synced with AWS for a given user. The tool attempts to assume the Role and, if successful, stores the temporary credentials as a profile section in the `~/.aws/credentials`. The user can then set their `AWS_PROFILE` env accordingly and work with the privileges of the Role.
