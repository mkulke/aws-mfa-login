# AWS MFA Login

## Caution

Access management in AWS is pretty hairy and easy to mess up. Credentials can be leaked by accident and accounts abused by crypto enthusiasts. Please do not deploy/run the code in the repository without a clear understanding of what's going on under the hood. The code in this repository is meant to provide inspiration and to explore the recently released aws-rust-sdk.

## Why

- We want to enforce MFA for users in an AWS account.
- Users should assume roles with privileges requried for specific workloads, instead of having static user/group privileges.
- Temporary credentials should be cached (boto3 does this, but not all aws sdks do. notably the js sdk and hence CDK acquire new tokens for every action, which in combination with MFA is pretty nasty).

## How

We have an iam Role which has certain policies attached (`AdministratorAccess` in our case). This role can only be assumed if the user has been authenticated with a MFA device (`"Bool": { "aws:MultiFactorAuthPresent": true }`).

There is an iam Group in which we can put users. The users are allow to assume the Role above, the constraints of the to-be-assumed role (MFA required) still have to be considered.

A iam Policy which is assigned to the Group, to allow users in the group to manage their MFA user settings, however they cannot remove/disable a MFA device (and thus lock themselves out).

In `./cfn` there is a CloudFormation template to create the above resources, based on a template from [Mattias Severson](https://blog.jayway.com/2017/11/22/aws-cli-mfa):

```bash
aws cloudformation deploy \
    --template-file cfn/aws-cli-mfa.yaml \
    --stack-name aws-cli-mfa \
    --capabilities CAPABILITY_NAMED_IAM
```

The cli tool `aws-mfa-login` is invoked with a `--token`, which is retrieved from an MFA device that has been synced with AWS for a given user. The tool attempts to assume the Role and, if successful, stores the temporary credentials as a profile section in the `~/.aws/credentials`. The user can then set their `AWS_PROFILE` env accordingly and work with the privileges of the Role.

In essence, the tool performs a call to `aws sts assume-role ...` and populates a profile entry to `~/.aws/credentials` with the temporary credentials.

## Build

The code has been built and tested with Rust 1.57.

```
cargo build --release
```

## Configure

Static settings need to be specified in a configuration file:

```bash
cat ~/.aws-mfa-login.toml
role-arn = "arn:aws:iam::REDACTED:role/AdminMFARole"
mfa-serial-number = "arn:aws:iam::REDACTED:mfa/AdminMFADevice"
session-name = "admin-session"
aws-profile = "some-profile"
```

## Run

A token should be specified when the tool is invoked:

```
cargo run --release -- -t 123456
export AWS_PROFILE=some-profile
aws sts get-caller-identity
{
    "UserId": "ABOAX5HIYTM6IBZL6DS3B:admin-session",
    "Account": "REDACTED",
    "Arn": "arn:aws:sts::REDACTED:assumed-role/AdminMFARole/admin-session"
}
```
