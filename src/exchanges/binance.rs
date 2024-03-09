use super::*;

use futures_util::TryFutureExt;
use hmac::{Hmac, Mac};

use serde::Deserialize;
use sha2::Sha256;

const TIME_OFFSET_MS_BINANCE: u64 = 36880;

pub struct Binance {
    api_key: String,
    api_sec: String,
}

impl Binance {
    pub fn get_api_key(&self) -> &String {
        &self.api_key
    }
    pub fn get_api_sec(&self) -> &String {
        &self.api_sec
    }
}

impl Client for Binance {
    fn new(api_key: String, api_sec: String) -> Binance {
        BinanceConfig::set_binance_api_credentials(api_key.clone(), api_sec.clone());
        Binance { api_key, api_sec }
    }
}

impl WebsocketClient for Binance {
    fn ws_url(&self) -> String {
        "wss://stream.binance.com:9443/stream".to_string()
    }

    fn subscription_message(&self, asset: &str) -> String {
        let asset_without_slash = asset.replace("/", "");
        let subscription_params = format!("{}@trade", asset_without_slash.to_lowercase());
        serde_json::json!({
            "method": "SUBSCRIBE",
            "params": [
                subscription_params
            ],
        })
        .to_string()
    }
}

impl RestClient for Binance {
    async fn query(method: &str, url_encoded_body: &str) -> String {
        BinanceClient::get_binance_api_response(method.to_string(), url_encoded_body.to_string())
            .await
    }
}

struct BinanceClient;

impl BinanceClient {
    fn get_signature(query_string: &str) -> String {
        let api_secret: String = BinanceConfig::get_binance_api_secret();
        let key = api_secret.as_bytes();
        let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC can take key of any size");
        mac.update(query_string.as_bytes());
        let result = mac.finalize();
        let code_bytes = result.into_bytes();
        hex::encode(code_bytes)
    }

    async fn get_server_time() -> Result<i64, Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let res = client
            .get("https://api.binance.com/api/v3/time")
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        Ok(res["serverTime"].as_i64().unwrap())
    }

    pub async fn api_request(
        method: &str,
        url_encoded_body: &str,
    ) -> Result<String, reqwest::Error> {
        let method_type = BinanceUtils::get_method_type(method);
        let api_path = format!(
            "/api/{}/{}",
            BinanceConfig::get_binance_api_version(),
            method
        );
        let mut api_endpoint = format!("{}{}", BinanceConfig::get_binance_api_url(), api_path);
        let api_timeout = BinanceConfig::get_binance_api_timeout();

        let client = reqwest::Client::new();
        match method_type {
            //???
            "public" => {
                if !url_encoded_body.is_empty() {
                    api_endpoint = format!("{}?{}", api_endpoint, url_encoded_body);
                }
                let response = client
                    .get(&api_endpoint)
                    .timeout(api_timeout)
                    .send()
                    .await?;
                response.text().await
            }
            "private" => {
                let server_time = BinanceClient::get_server_time().await.unwrap();
                let mut signature = String::new();

                if !url_encoded_body.is_empty() {
                    api_endpoint = format!("{}?{}", api_endpoint, url_encoded_body);

                    let query_string = format!("{}&timestamp={}", url_encoded_body, server_time);
                    signature = BinanceClient::get_signature(&query_string);
                } else {
                    let query_string = format!("timestamp={}", server_time);
                    signature = BinanceClient::get_signature(&query_string);
                }

                let client = reqwest::Client::new();
                let res = client
                    .get(api_endpoint)
                    .query(&[
                        ("timestamp", server_time.to_string().as_str()),
                        ("signature", signature.as_str()),
                    ])
                    .header("X-MBX-APIKEY", BinanceConfig::get_binance_api_key())
                    .send()
                    .await?;
                res.text().await
            }
            _ => {
                panic!("Invalid method type");
            }
        }
    }

    // let account_info: AccountInfo = res.json()?;
    // println!("{:#?}", account_info);

    // Ok(())
    pub async fn get_binance_api_response(api_method: String, url_encoded_body: String) -> String {
        match BinanceClient::api_request(&api_method, &url_encoded_body).await {
            Ok(result) => result,
            Err(error) => error.to_string(),
        }
    }
}

//binance conf

static mut BINANCE_API_KEY: String = String::new();
static mut BINANCE_API_SECRET: String = String::new();
const BINANCE_API_URL: &str = "https://api.binance.com";
const BINANCE_API_VERSION: &str = "v3";
const BINANCE_API_TIMEOUT: u64 = 5000;

use std::time::Duration;

struct BinanceConfig;

impl BinanceConfig {
    pub fn set_binance_api_credentials(api_key: String, api_secret: String) {
        BinanceConfig::set_binance_api_key(api_key);
        BinanceConfig::set_binance_api_secret(api_secret);
    }

    pub fn set_binance_api_key(api_key: String) {
        unsafe {
            BINANCE_API_KEY = api_key;
        }
    }

    pub fn get_binance_api_key() -> String {
        unsafe { BINANCE_API_KEY.to_string() }
    }

    pub fn set_binance_api_secret(api_secret: String) {
        unsafe {
            BINANCE_API_SECRET = api_secret;
        }
    }

    pub fn get_binance_api_secret() -> String {
        unsafe { BINANCE_API_SECRET.to_owned() }
    }

    pub fn get_binance_api_url() -> String {
        BINANCE_API_URL.to_string()
    }

    pub fn get_binance_api_version() -> String {
        BINANCE_API_VERSION.to_string()
    }

    pub fn get_binance_api_timeout() -> Duration {
        Duration::from_millis(BINANCE_API_TIMEOUT)
    }
}

#[derive(Debug, Deserialize)]
struct BinanceTime {
    serverTime: i64,
}

pub struct BinanceUtils;

impl BinanceUtils {
    pub fn is_method_public(method: &str) -> bool {
        ["Time"].contains(&method)
    }

    pub fn is_method_private(method: &str) -> bool {
        ["account", "myTrades"].contains(&method)
    }

    pub fn get_method_type(method: &str) -> &str {
        let method_type: &str = if BinanceUtils::is_method_public(method) {
            "public"
        } else if BinanceUtils::is_method_private(method) {
            "private"
        } else {
            "invalid"
        };
        method_type
    }

    //time utils
    pub fn get_time_offset_millis() -> u64 {
        TIME_OFFSET_MS_BINANCE
    }

    async fn binance_time_millis() -> Result<i64, reqwest::Error> {
        let response = reqwest::get("https://api.binance.com/api/v3/time")
            .await?
            .json::<BinanceTime>()
            .await?;

        Ok(response.serverTime)
    }

    // Make get_time_difference async and handle asynchronous call to binance_time_millis
    pub async fn get_time_difference() -> Result<i64, Box<dyn std::error::Error>> {
        let local_time = chrono::Utc::now().timestamp_millis();
        let server_time = Self::binance_time_millis().await?;

        Ok(server_time - local_time)
    }
}
