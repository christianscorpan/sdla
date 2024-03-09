use core::time;

use crate::{balance::{Balance, BalanceBuffer}, tick::{Tick, TickBuffer}, BUFF_SIZE};

//gap arg from 0 to 1
//fee_slow_buff arg from 0 to 1
//trade_size arg from 0 to 1
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn oneleg(
    norm_trade_size: f64,
    norm_gap: f64,
    time_gap_ms: usize,
    max_time_diff_ms: usize,
    fee_slow_buff: f64,
    fast_buff_back: &Tick,
    slow_buff_back: &Tick,
    balance_buff_back: &Balance,
)
{
    //get time difference between the two exchanges

    let time_diff_ms = fast_buff_back.timestamp2 as i64 - slow_buff_back.timestamp2 as i64;
    let time_diff_ms = time_diff_ms.abs();
    let time_diff_ms = time_diff_ms as usize;


    if (time_diff_ms < max_time_diff_ms) {
        // println!("time diff good: {}", time_diff_ms);

        let price_diff = fast_buff_back.avg - slow_buff_back.avg;

        //get ratio of the exchanges' price
        let ratio: f64 = fast_buff_back.avg / slow_buff_back.avg;


        //test
        if (ratio > 1.0) {
            println!("ratio: {}", ratio-1.0);
        }
        else {
            println!("ratio: {}", 1.0-ratio);
        }

        // if difference is greater than gap + fee, then trade
        if (ratio > 1. + norm_gap + fee_slow_buff || ratio < 1. - norm_gap - fee_slow_buff) {

            println!("ratio pass");

            let denorm_trade_size = norm_trade_size * balance_buff_back.amount;

            //if difference is positive, then bull slow
            if price_diff > 0.0 {
                //buy
                println!("buying for {}", denorm_trade_size);

                //wait time_gap_ms

                //sell max amount of the bought asset
            }
            //if difference is negative, then bear slow
            else {
                //sell

                println!("selling for {}", denorm_trade_size);

                //wait time_gap_ms

                //buy max amount of the sold asset
            }
        }
    }
}
