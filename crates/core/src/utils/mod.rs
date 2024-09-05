use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};

pub mod async_util;
pub mod float;
pub mod json;
pub mod json_seq;
pub mod parser;
pub mod time;

pub fn generate_uid() -> u64 {
    let mut rng = rand::thread_rng();
    let random_part: u64 = rng.gen();
    let timestamp_part = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time error!!!")
        .as_nanos() as u64;

    timestamp_part ^ random_part
}

pub fn generate_str_uid() -> String {
    format!("{:016x}", generate_uid())
}
