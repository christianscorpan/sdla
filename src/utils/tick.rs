use std::{fs::File, io::Write};

use anyhow::{anyhow, Result};
use circular_buffer::CircularBuffer;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tick {
    pub timestamp: u64,
    pub timestamp2: u64,
    pub avg: f64,
    pub exchange: String,
    pub asset: String,
}
#[derive(Debug, Clone)]
pub struct TickBuffer<const SIZE: usize> {
    pub buffer: CircularBuffer<SIZE, Tick>,
    pub exchange: String,
}

impl Tick {
    pub fn deserialize_tick_kraken(
        json_string: &str,
        asset: String,
        timestamp2: u64,
    ) -> Result<Tick, anyhow::Error> {
        let value: serde_json::Value = serde_json::from_str(json_string)
            .map_err(|e| anyhow!("Failed to parse JSON: {}", e))?;

        // Assuming the format is always as shown, with prices at specific indices
        if let Some(array) = value.as_array() {
            // Safely access elements within the nested structure
            if array.len() > 1 && array[1].is_array() {
                let prices = &array[1].as_array().unwrap();

                if prices.len() >= 2 {
                    let bid: f64 = prices[0]
                        .as_str()
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| anyhow!("Invalid format for bid price"))?;
                    let ask: f64 = prices[1]
                        .as_str()
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| anyhow!("Invalid format for ask price"))?;

                    let avg = (bid + ask) / 2.0;

                    // Construct the Tick object here with 'avg' and other necessary fields
                    return Ok(Tick {
                        exchange: "Kraken".to_string(),
                        timestamp: 0, // Set appropriate timestamp
                        avg,
                        asset,
                        timestamp2,
                    });
                }
            }
        }

        Err(anyhow!("Invalid format for Kraken message"))
    }

    pub fn deserialize_tick_binance(message: &str, asset: String, timestamp2: u64) -> Result<Tick> {
        let value = serde_json::from_str::<Value>(message)
            .map_err(|e| anyhow!("Deserialization error: {}", e))?;
        let data = value.get("data").ok_or_else(|| anyhow!("Missing data"))?;

        let price = data["p"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing price"))?
            .parse::<f64>()
            .map_err(|_| anyhow!("Invalid price format"))?;
        let timestamp = data["T"]
            .as_u64()
            .ok_or_else(|| anyhow!("Missing timestamp"))?;

        Ok(Tick {
            exchange: "Binance".to_string(),
            avg: price,
            timestamp,
            timestamp2,
            asset,
        })
    }
}

impl<const SIZE: usize> TickBuffer<SIZE> {
    pub fn new(exchange: String) -> TickBuffer<SIZE> {
        TickBuffer {
            buffer: CircularBuffer::<SIZE, Tick>::new(),
            exchange: exchange,
        }
    }

    pub fn add_tick(&mut self, tick: Tick) {
        if self.exchange == tick.exchange {
            self.buffer.push_back(tick);
        }
    }

    pub fn serialize_to_json(&self) -> Result<String, serde_json::Error> {
        let ticks: Vec<&Tick> = self.buffer.iter().collect();
        serde_json::to_string(&ticks)
    }

    pub fn save_to_file(&self, file_path: &str) -> std::io::Result<()> {
        let json_string = self
            .serialize_to_json()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        //handle case if directory does not exist

        let mut file = File::create(file_path)?;
        file.write_all(json_string.as_bytes())?;
        Ok(())
    }
}
