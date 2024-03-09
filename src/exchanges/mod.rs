// Exchange trait and Kraken implementation
pub mod binance;
pub mod kraken;

pub trait Client {
    fn new(api_key: String, api_sec: String) -> Self;
}

pub trait WebsocketClient {
    fn ws_url(&self) -> String;
    fn subscription_message(&self, asset: &str) -> String;
}

pub trait RestClient {
    async fn query(method: &str, url_encoded_body: &str) -> String;
}

// Message struct for channel communication
pub struct ExchangeMessage {
    pub sender: String,
    pub asset: String,
    pub content: String,
}
