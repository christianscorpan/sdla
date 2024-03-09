#![allow(warnings)]
pub mod exchanges;
pub mod strats;
pub mod utils;

use std::sync::Arc;
use tokio::sync::RwLock;

use exchanges::binance::BinanceUtils;
use exchanges::kraken::Kraken;
use futures_util::{SinkExt, StreamExt, TryFutureExt};
use serde_json::to_string;
use strats::oneleg;
use tokio::sync::mpsc;
use tokio::task;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use utils::balance::BalanceBuffer;
use utils::*;

use crate::exchanges::binance::Binance;
use crate::exchanges::{Client, ExchangeMessage, RestClient, WebsocketClient};
use crate::tick::{Tick, TickBuffer};
//buffer const
const BUFF_SIZE: usize = 100;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<ExchangeMessage>(100);

    let tick_buffer_kraken = Arc::new(RwLock::new(TickBuffer::<BUFF_SIZE>::new(
        "Kraken".to_string(),
    )));
    let tick_buffer_binance = Arc::new(RwLock::new(TickBuffer::<BUFF_SIZE>::new(
        "Binance".to_string(),
    )));
    let balance_buffer_kraken = Arc::new(RwLock::new(BalanceBuffer::<BUFF_SIZE>::new(
        "Kraken".to_string(),
    )));

    //load api keys from file

    let (api_key_kraken, api_sec_kraken) =
        utils::api_key_man::read_api_credentials_from_file("src/config/kraken_api_key").unwrap();

    let (api_key_binance, api_sec_binance) =
        utils::api_key_man::read_api_credentials_from_file("src/config/binance_api_key").unwrap();

    //setup clients with credentials
    let kraken = Kraken::new(api_key_kraken, api_sec_kraken);

    let binance = Binance::new(api_key_binance, api_sec_binance);

    let asset_kraken = "PEPE/USD";
    let asset_binance = "PEPE/USDT";

    let kraken_tx = tx.clone();
    task::spawn(async move {
        connect_and_run("Kraken", kraken, kraken_tx, asset_kraken).await;
    });

    let binance_tx = tx.clone();
    task::spawn(async move {
        connect_and_run("Binance", binance, binance_tx, asset_binance).await;
    });

    // Clone for the first async block (e.g., periodic REST API query for Kraken)
    let balance_buffer_kraken_clone_write = balance_buffer_kraken.clone();

    //clone fore writes for binance and kraken buffers
    let tick_buffer_kraken_clone_write = tick_buffer_kraken.clone();
    let tick_buffer_binance_clone_write = tick_buffer_binance.clone();

    // Setup periodic REST API query for Kraken
    task::spawn(async move {
        //wait 1 second interval
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let response = Kraken::query("Balance", "").await;
            //extract balance
            let balance_kraken_struct = balance::Balance::extract_balance_kraken(&response, "USDT")
                .expect("Failed to extract balance");

            //push to buffer
            {
                let mut balance_buffer = balance_buffer_kraken_clone_write.write().await;
                balance_buffer.add_balance(balance_kraken_struct.clone());
            }

            // println!("Kraken balance: {:?}", balance_kraken_struct);
        }
    });

    // Task for trading logic and execution
    task::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(10));
        // Clone buffers for use in async tasks

        loop {
            interval.tick().await;

            let tick_buffer_binance_read = tick_buffer_binance.read().await;
            let tick_buffer_kraken_read = tick_buffer_kraken.read().await;
            let balance_buffer_kraken_read = balance_buffer_kraken.read().await;

            // Using match
            let fast_buff_back = match tick_buffer_binance_read.buffer.back() {
                Some(back) => back,
                None => {
                    eprintln!(
                        "Error: Could not get back element from tick_buffer_binance_read.buffer"
                    );
                    continue;
                }
            };

            let slow_buff_back = match tick_buffer_kraken_read.buffer.back() {
                Some(back) => back,
                None => {
                    eprintln!(
                        "Error: Could not get back element from tick_buffer_kraken_read.buffer"
                    );
                    continue;
                }
            };

            let balance_buffer_back = match balance_buffer_kraken_read.buffer.back() {
                Some(back) => back,
                None => {
                    eprintln!(
                        "Error: Could not get back element from balance_buffer_kraken_read.buffer"
                    );
                    continue;
                }
            };

            oneleg::oneleg(
                0.01,
                0.00001,
                1000,
                100,
                0.0026,
                &fast_buff_back,
                &slow_buff_back,
                &balance_buffer_back,
            )
            .await;
        }
    });

    // Process incoming messages
    while let Some(message) = rx.recv().await {
        // println!("Received message from {}: {}", message.sender, message.content);
        //get timestamp2 in ms since epoch using chrono as u64
        let timestamp2 =
            chrono::Utc::now().timestamp_millis() as u64 - BinanceUtils::get_time_offset_millis();

        match message.sender.as_str() {
            "Kraken" => {
                match Tick::deserialize_tick_kraken(&message.content, message.asset, timestamp2) {
                    Ok(tick) => {
                        // println!("tick: {:?}", tick.clone());

                        let mut tick_buffer = tick_buffer_kraken_clone_write.write().await;
                        tick_buffer.add_tick(tick);
                    }
                    Err(e) => {
                        // eprintln!("Failed to deserialize Kraken tick: {}\n The message content is: {}", e, &message.content);
                    }
                }
            }
            "Binance" => {
                match Tick::deserialize_tick_binance(&message.content, message.asset, timestamp2) {
                    Ok(tick) => {
                        // println!("tick: {:?}", tick.clone());
                        let mut tick_buffer = tick_buffer_binance_clone_write.write().await;
                        tick_buffer.add_tick(tick);
                    }
                    Err(e) => {
                        // eprintln!("Failed to deserialize Binance tick: {}\n The message content is: {}", e, &message.content);
                    }
                }
            }
            _ => {
                println!("Unknown exchange: {}", message.sender);
            }
        }
    }
}

async fn connect_and_run<T: WebsocketClient>(
    exchange_name: &str,
    exchange: T,
    tx: mpsc::Sender<ExchangeMessage>,
    asset: &str,
) {
    let (ws_stream, _) = connect_async(exchange.ws_url())
        .await
        .expect("Failed to connect");
    println!("WebSocket connected to {}", exchange_name);

    let (mut write, mut read) = ws_stream.split();

    let subscribe_message = exchange.subscription_message(asset);
    write
        .send(Message::Text(subscribe_message))
        .await
        .expect("Failed to send subscribe message");

    while let Some(message) = read.next().await {
        if let Ok(Message::Text(text)) = message {
            let exchange_msg = ExchangeMessage {
                sender: exchange_name.to_string(),
                asset: asset.to_string(),
                content: text,
            };
            if tx.send(exchange_msg).await.is_err() {
                eprintln!("Failed to send message from {}", exchange_name);
                break;
            }
        }
    }
}
