use aws_config::meta::region::RegionProviderChain;
use aws_sdk_sts::output::AssumeRoleOutput;
use aws_sdk_sts::{Client, SdkError};
use configparser::ini::Ini;
use figment::providers::{Format, Toml};
use figment::Figment;
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use std::error::Error;
use std::path::PathBuf;
use structopt::StructOpt;

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Debug, StructOpt)]
struct Opt {
    /// MFA token code
    #[structopt(short, long)]
    token: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Config {
    role_arn: String,
    mfa_serial_number: String,
    session_name: String,
    aws_profile: String,
}

#[derive(Debug)]
struct Credentials {
    access_key_id: String,
    secret_access_key: String,
    session_token: String,
}

impl TryFrom<AssumeRoleOutput> for Credentials {
    type Error = &'static str;

    fn try_from(value: AssumeRoleOutput) -> Result<Self, Self::Error> {
        value
            .credentials
            .and_then(|o| {
                let access_key_id = o.access_key_id?;
                let secret_access_key = o.secret_access_key?;
                let session_token = o.session_token?;
                let credentials = Credentials {
                    access_key_id,
                    secret_access_key,
                    session_token,
                };
                Some(credentials)
            })
            .ok_or("could not extract credentials")
    }
}

fn set_credentials(ini: &mut Ini, profile: &str, credentials: Credentials) {
    ini.set(
        profile,
        "aws_access_key_id",
        Some(credentials.access_key_id),
    );
    ini.set(
        profile,
        "aws_secret_access_key",
        Some(credentials.secret_access_key),
    );
    ini.set(
        profile,
        "aws_session_token",
        Some(credentials.session_token),
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let Opt { token } = Opt::from_args();

    let home_dir = dirs::home_dir().ok_or("could not get home directory")?;
    let config_path: PathBuf = [home_dir.clone(), format!(".{}.toml", PKG_NAME).into()]
        .iter()
        .collect();
    let Config {
        role_arn,
        mfa_serial_number,
        session_name,
        aws_profile,
    } = Figment::new().merge(Toml::file(&config_path)).extract()
        .map_err(|e| {
            format!("{}: {}", &config_path.to_string_lossy(), e)
        })?;

    let region = RegionProviderChain::default_provider()
        .region()
        .await
        .ok_or("default region unset")?;

    let config = aws_config::from_env().region(region).load().await;
    let client = Client::new(&config);

    let assume_role_output = client
        .assume_role()
        .serial_number(mfa_serial_number)
        .token_code(token)
        .role_arn(role_arn)
        .role_session_name(session_name)
        .send()
        .await
        .map_err(|e| {
            if let SdkError::ServiceError { err, .. } = e {
                if let Some(message) = err.message() {
                    return String::from(message);
                }
            }
            String::from("unknown error")
        })?;
    let credentials: Credentials = assume_role_output.try_into()?;

    let mut credentials_ini = Ini::new();
    let credentials_path: PathBuf = [home_dir, ".aws".into(), "credentials".into()]
        .iter()
        .collect();
    credentials_ini.set_default_section("");
    credentials_ini.load(&credentials_path)?;
    set_credentials(&mut credentials_ini, &aws_profile, credentials);
    credentials_ini.write(&credentials_path)?;

    Ok(())
}
