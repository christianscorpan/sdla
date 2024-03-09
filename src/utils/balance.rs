use std::{error::Error, fs::File, io::Write};

use circular_buffer::CircularBuffer;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Balance {
    pub currency: String,
    pub amount: f64,
    pub exchange: String,
}
#[derive(Debug, Clone)]
pub struct BalanceBuffer<const SIZE: usize> {
    pub buffer: CircularBuffer<SIZE, Balance>,
    pub exchange: String,
}

impl Balance {
    pub fn extract_balance_binance(
        json_string: &str,
        asset: &str,
    ) -> Result<Balance, Box<dyn Error>> {
        let json_value: Value = serde_json::from_str(json_string)?;
        let balance_str = json_value["balances"]
            .as_array()
            .ok_or("Failed to get account balances from API")?
            .iter()
            .find(|&balance| balance["asset"].as_str() == Some(asset))
            .ok_or(format!("Asset not found: {}", asset))?
            .get("free")
            .and_then(|v| v.as_str())
            .ok_or(format!("Failed to get balance for asset: {}", asset))?
            .to_string();

        let balance: f64 = balance_str.parse()?;

        Ok(Balance {
            currency: asset.to_string(),
            amount: balance,
            exchange: "Binance".to_string(),
        })
    }

    pub fn extract_balance_kraken(json_str: &str, asset: &str) -> Result<Balance, Box<dyn Error>> {
        let v: Value = serde_json::from_str(json_str)?;
        let balance = v["result"][asset].as_str();
        ////parse balance as f64
        let balance_str = match balance {
            Some(b) => match b.parse::<String>() {
                Ok(balance) => balance,
                Err(e) => return Err(e.into()),
            },
            None => {
                eprintln!("Error: balance is None");
                let error_message = format!("Balance is None\n The json for the asset {} is: {}", asset, json_str);
                return Err(error_message.into());
            }
        };
        
        let balance: f64 = match balance_str.parse::<f64>() {
            Ok(balance) => balance,
            Err(e) => return Err(e.into()),
        };
        if let Some(balance) = Some(balance) {
            Ok(Balance {
                currency: asset.to_string(),
                amount: balance,
                exchange: "Kraken".to_string(),
            })
        } else {
            Err("Failed to parse balance".into())
        }
    }
}

impl<const SIZE: usize> BalanceBuffer<SIZE> {
    pub fn new(exchange: String) -> BalanceBuffer<SIZE> {
        BalanceBuffer {
            buffer: CircularBuffer::<SIZE, Balance>::new(),
            exchange: exchange,
        }
    }

    pub fn add_balance(&mut self, balance: Balance) {
        if self.exchange == balance.exchange {
            self.buffer.push_back(balance);
        }
    }

    pub fn serialize_to_json(&self) -> Result<String, serde_json::Error> {
        let balances: Vec<&Balance> = self.buffer.iter().collect();
        serde_json::to_string(&balances)
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
