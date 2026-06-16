use dotenv::dotenv;
use serde::Deserialize;
use std::env;

pub struct Config {
    pub db_url: String,
    pub db_user: String,
    pub db_pass: String,
    pub db_ns: String,
    pub db_name: String,
}

#[derive(Debug, Deserialize)]
struct VaultKvV2Response {
    data: VaultKvV2Data,
}

#[derive(Debug, Deserialize)]
struct VaultKvV2Data {
    data: serde_json::Map<String, serde_json::Value>,
}

pub async fn load() -> anyhow::Result<Config> {
    dotenv().ok();
    load_vault_secrets().await?;

    Ok(Config {
        db_url: env::var("SURREAL_DB_URL").expect("Missing SURREAL_DB_URL"),
        db_user: env::var("SURREAL_DB_USER").expect("Missing SURREAL_DB_USER"),
        db_pass: env::var("SURREAL_DB_PASS").expect("Missing SURREAL_DB_PASS"),
        db_ns: env::var("SURREAL_DB_NS").expect("Missing SURREAL_DB_NS"),
        db_name: env::var("SURREAL_DB_DB").expect("Missing SURREAL_DB_DB"),
    })
}

async fn load_vault_secrets() -> anyhow::Result<()> {
    let vault_addr = match env::var("VAULT_ADDR") {
        Ok(value) if !value.is_empty() => value.trim_end_matches('/').to_string(),
        _ => return Ok(()),
    };

    let vault_token = match env::var("VAULT_TOKEN") {
        Ok(value) if !value.is_empty() => value,
        _ => return Ok(()),
    };

    let secret_path = env::var("VAULT_SECRET_PATH")
        .unwrap_or_else(|_| "secret/expense-tracker-be-local".to_string());
    let (mount, path) = secret_path.split_once('/').ok_or_else(|| {
        anyhow::anyhow!("VAULT_SECRET_PATH must look like secret/expense-tracker-be-local")
    })?;

    let url = format!("{}/v1/{}/data/{}", vault_addr, mount, path);
    let response = reqwest::Client::new()
        .get(url)
        .header("X-Vault-Token", vault_token)
        .send()
        .await?
        .error_for_status()?
        .json::<VaultKvV2Response>()
        .await?;

    for (key, value) in response.data.data {
        if let Some(value) = value.as_str() {
            env::set_var(key, value);
        }
    }

    Ok(())
}
