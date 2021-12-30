use aws_config::meta::region::RegionProviderChain;
use aws_sdk_sts::output::AssumeRoleOutput;
use aws_sdk_sts::{Client, SdkError};
use configparser::ini::Ini;
use figment::providers::{Format, Toml};
use figment::Figment;
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use std::error::Error;
use std::path::{Path, PathBuf};
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

fn set_credentials(
    config: &Config,
    home_dir: &Path,
    credentials: Credentials,
) -> Result<(), Box<dyn Error>> {
    let ini_path: PathBuf = [home_dir.to_path_buf(), ".aws".into(), "credentials".into()]
        .iter()
        .collect();
    let mut ini = Ini::new();
    ini.set_default_section("");
    ini.load(&ini_path)?;

    let Config { aws_profile, .. } = config;

    let mut set = |k, v| ini.set(aws_profile, k, Some(v));
    set("aws_access_key_id", credentials.access_key_id);
    set("aws_secret_access_key", credentials.secret_access_key);
    set("aws_session_token", credentials.session_token);
    ini.write(&ini_path)?;

    Ok(())
}

async fn assume_role(config: &Config, token: &str) -> Result<Credentials, Box<dyn Error>> {
    let region = RegionProviderChain::default_provider()
        .region()
        .await
        .ok_or("default region unset")?;

    let client_config = aws_config::from_env().region(region).load().await;
    let client = Client::new(&client_config);

    let Config {
        role_arn,
        mfa_serial_number,
        session_name,
        ..
    } = config;

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

    Ok(credentials)
}

fn get_config(home_dir: &Path) -> Result<Config, Box<dyn Error>> {
    let config_path: PathBuf = [home_dir.to_path_buf(), format!(".{}.toml", PKG_NAME).into()]
        .iter()
        .collect();

    let config: Config = Figment::new()
        .merge(Toml::file(&config_path))
        .extract()
        .map_err(|e| format!("{}: {}", &config_path.to_string_lossy(), e))?;

    Ok(config)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let Opt { token } = Opt::from_args();

    let home_dir = dirs::home_dir().ok_or("could not resolve home directory")?;
    let config = get_config(&home_dir)?;
    let credentials = assume_role(&config, &token).await?;
    set_credentials(&config, &home_dir, credentials)?;

    Ok(())
}
