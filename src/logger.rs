use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(format!(
        "{}/Library/Application Support/MacXtreamer",
        home
    ))
}

pub fn log_path() -> PathBuf {
    let dir = data_dir();
    let _ = fs::create_dir_all(&dir);
    dir.join("macxtreamer.log")
}

fn timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", now)
}

pub fn log_line(line: &str) {
    let path = log_path();
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[{}] {}", timestamp(), line);
    }
}

pub fn log_error(prefix: &str, e: &dyn std::error::Error) {
    log_line(&format!("ERROR: {}: {}", prefix, e));
}

pub fn log_command(program: &str, args: &[String]) {
    let joined = args.join(" ");
    log_line(&format!("RUN: {} {}", program, joined));
}
