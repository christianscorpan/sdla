use std::fs::File;
use std::io::{self, BufRead, BufReader};

pub(crate) fn read_api_credentials_from_file(file_path: &str) -> io::Result<(String, String)> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let mut api_key = String::new();
    let mut api_sec = String::new();

    for line in reader.lines() {
        let line = line?;
        if line.starts_with("api_key:") {
            api_key = line.trim_start_matches("api_key:").trim().to_string();
        } else if line.starts_with("api_sec:") {
            api_sec = line.trim_start_matches("api_sec:").trim().to_string();
        }
    }

    Ok((api_key, api_sec))
}
