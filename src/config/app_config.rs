use dotenv::dotenv;
use std::env;

pub struct Config {
    pub db_url: String,
    pub db_user: String,
    pub db_pass: String,
    pub db_ns: String,
    pub db_name: String,
}

pub fn load() -> Config {
    dotenv().ok();
    Config {
        db_url: env::var("SURREAL_DB_URL").expect("Missing SURREAL_DB_URL"),
        db_user: env::var("SURREAL_DB_USER").expect("Missing SURREAL_DB_USER"),
        db_pass: env::var("SURREAL_DB_PASS").expect("Missing SURREAL_DB_PASS"),
        db_ns: env::var("SURREAL_DB_NS").expect("Missing SURREAL_DB_NS"),
        db_name: env::var("SURREAL_DB_DB").expect("Missing SURREAL_DB_DB"),
    }
}
