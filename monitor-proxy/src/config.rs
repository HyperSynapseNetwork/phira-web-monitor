use clap::Parser;
use std::{env, net::IpAddr, path::PathBuf};

fn default_cache_path() -> PathBuf {
    let mut path: PathBuf = env::var_os("HOME").unwrap_or(".".into()).into();
    path.push(".cache");
    path.push("hsn-phira");
    path
}

#[derive(Parser, Debug, Clone)]
#[command(name = "monitor-proxy", about = "Phira Web Monitor Proxy Server")]
pub struct Config {
    /// Debug mode
    #[arg(long)]
    pub debug: bool,

    /// host to listen on
    #[arg(long, default_value = "0.0.0.0")]
    pub host: IpAddr,

    /// Port to listen on
    #[arg(long, default_value_t = 3080)]
    pub port: u16,

    /// Directory for disk-based chart cache
    #[arg(long, default_value_os_t = default_cache_path())]
    pub cache_dir: PathBuf,

    /// Phira API base URL
    #[arg(long, default_value = "https://phira.5wyxi.com")]
    pub api_base: String,

    /// Phira-mp server address
    #[arg(long, default_value = "localhost:12346")]
    pub mp_server: String,

    /// Allowed CORS origin (used when --debug is not set)
    #[arg(long)]
    pub allowed_origin: Option<String>,
}
