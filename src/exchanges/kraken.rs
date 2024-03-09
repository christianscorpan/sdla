use super::*;

use hmac::{Hmac, Mac};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Error,
};
use sha2::{Digest, Sha256, Sha512};
use std::time::{SystemTime, UNIX_EPOCH};
type HmacSha512 = Hmac<Sha512>;

pub struct Kraken {
    api_key: String,
    api_sec: String,
}

impl Client for Kraken {
    fn new(api_key: String, api_sec: String) -> Kraken {
        KrakenConfig::set_kraken_api_credentials(api_key.clone(), api_sec.clone());

        Kraken { api_key, api_sec }
    }
}

impl WebsocketClient for Kraken {
    fn ws_url(&self) -> String {
        "wss://ws.kraken.com".to_string()
    }

    fn subscription_message(&self, asset: &str) -> String {
        serde_json::json!({
            "event": "subscribe",
            "pair": [
                asset
            ],
            "subscription": {"name": "spread"}
        })
        .to_string()

    
    }
}

impl RestClient for Kraken {
    async fn query(method: &str, url_encoded_body: &str) -> String {
        KrakenClient::get_kraken_api_response(method.to_string(), url_encoded_body.to_string())
            .await
    }
}

struct KrakenClient;

impl KrakenClient {
    fn get_signature(
        api_path: String,
        nonce: String,
        url_encoded_body: String,
        api_secret: String,
    ) -> String {
        // API-Sign = Message signature using HMAC-SHA512 of (URI path + SHA256(nonce + POST data)) and base64 decoded secret API key
        let hash_digest = Sha256::digest(format!("{}{}", nonce, url_encoded_body).as_bytes());
        let private_key = base64::decode(&api_secret).unwrap();
        let mut mac = HmacSha512::new_from_slice(&private_key).unwrap();

        let mut hmac_data = api_path.into_bytes();
        hmac_data.append(&mut hash_digest.to_vec());
        mac.update(&hmac_data);
        base64::encode(mac.finalize().into_bytes())
    }

    fn get_headers(signature: String) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "API-Key",
            HeaderValue::from_str(&KrakenConfig::get_kraken_api_key()).unwrap(),
        );
        headers.insert(
            "API-Sign",
            HeaderValue::from_str(&signature.to_string()).unwrap(),
        );
        headers
    }

    pub async fn api_request(method: &str, url_encoded_body: &str) -> Result<String, Error> {
        let method_type: &str = KrakenUtils::get_method_type(method);
        let api_path = format!(
            "/{}/{}/{}",
            KrakenConfig::get_kraken_api_version(),
            method_type,
            method
        );
        let mut api_endpoint = format!("{}{}", KrakenConfig::get_kraken_api_url(), api_path);
        let api_timeout = KrakenConfig::get_kraken_api_timeout();
        let api_response = match method_type {
            "public" => {
                if !url_encoded_body.is_empty() {
                    api_endpoint = api_endpoint + "?" + url_encoded_body;
                }
                reqwest::Client::new()
                    .get(&api_endpoint)
                    .timeout(api_timeout)
                    .send()
                    .await
            }
            "private" => {
                if KrakenConfig::get_kraken_api_key().is_empty()
                    || KrakenConfig::get_kraken_api_secret().is_empty()
                {
                    panic!("void credentials");
                }
                let nonce = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                let payload_nonce = format!("nonce={}", &nonce.to_string());
                let payload_body = if !url_encoded_body.is_empty() {
                    format!("{}&{}", payload_nonce, url_encoded_body)
                } else {
                    payload_nonce
                };
                let signature = KrakenClient::get_signature(
                    api_path,
                    nonce.to_string(),
                    payload_body.to_owned(),
                    KrakenConfig::get_kraken_api_secret(),
                );
                reqwest::Client::new()
                    .post(&api_endpoint)
                    .headers(KrakenClient::get_headers(signature))
                    .timeout(api_timeout)
                    .body(payload_body)
                    .send()
                    .await
            }
            _ => panic!("{} method is not supported", method),
        };
        match api_response {
            Ok(result) => result.text().await,
            Err(error) => Err(error),
        }
    }

    pub async fn get_kraken_api_response(api_method: String, url_encoded_body: String) -> String {
        match KrakenClient::api_request(&api_method, &url_encoded_body).await {
            Ok(result) => result,
            Err(error) => error.to_string(),
        }
    }
}

//kraken conf

static mut KRAKEN_API_KEY: String = String::new();
static mut KRAKEN_API_SECRET: String = String::new();
const KRAKEN_API_URL: &str = "https://api.kraken.com";
const KRAKEN_API_VERSION: &str = "0";
const KRAKEN_API_TIMEOUT: u64 = 5000;

use std::time::Duration;

struct KrakenConfig;

impl KrakenConfig {
    pub fn set_kraken_api_credentials(api_key: String, api_secret: String) {
        KrakenConfig::set_kraken_api_key(api_key);
        KrakenConfig::set_kraken_api_secret(api_secret);
    }

    pub fn set_kraken_api_key(api_key: String) {
        unsafe {
            KRAKEN_API_KEY = api_key;
        }
    }

    pub fn get_kraken_api_key() -> String {
        unsafe { KRAKEN_API_KEY.to_string() }
    }

    pub fn set_kraken_api_secret(api_secret: String) {
        unsafe {
            KRAKEN_API_SECRET = api_secret;
        }
    }

    pub fn get_kraken_api_secret() -> String {
        unsafe { KRAKEN_API_SECRET.to_owned() }
    }

    pub fn get_kraken_api_url() -> String {
        KRAKEN_API_URL.to_string()
    }

    pub fn get_kraken_api_version() -> String {
        KRAKEN_API_VERSION.to_string()
    }

    pub fn get_kraken_api_timeout() -> Duration {
        Duration::from_millis(KRAKEN_API_TIMEOUT)
    }
}

struct KrakenUtils;

impl KrakenUtils {
    pub fn is_method_public(method: &str) -> bool {
        [
            "Time",
            "Assets",
            "AssetPairs",
            "Ticker",
            "Depth",
            "Trades",
            "Spread",
            "OHLC",
        ]
        .contains(&method)
    }

    pub fn is_method_private(method: &str) -> bool {
        [
            "BalanceEx",
            "Balance",
            "TradeBalance",
            "OpenOrders",
            "ClosedOrders",
            "QueryOrders",
            "TradesHistory",
            "QueryTrades",
            "OpenPositions",
            "Ledgers",
            "QueryLedgers",
            "TradeVolume",
            "AddOrder",
            "CancelOrder",
            "DepositMethods",
            "DepositAddresses",
            "DepositStatus",
            "WithdrawInfo",
            "Withdraw",
            "WithdrawStatus",
            "WithdrawCancel",
            "GetWebSocketsToken",
        ]
        .contains(&method)
    }

    pub fn get_method_type(method: &str) -> &str {
        let method_type: &str = if KrakenUtils::is_method_public(method) {
            "public"
        } else if KrakenUtils::is_method_private(method) {
            "private"
        } else {
            "invalid"
        };
        method_type
    }
}
