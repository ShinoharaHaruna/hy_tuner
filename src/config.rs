use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tunable {
    pub test_file_url: String,
    pub latency_url: String,
    pub min_up: u32,
    pub max_up: u32,
    pub min_down: u32,
    pub max_down: u32,
    pub target_accuracy: u32,
    pub hy_config: PathBuf,
    pub hy_binary: PathBuf,
    pub hy_download_url: String,
}

impl Default for Tunable {
    fn default() -> Self {
        Self {
            test_file_url: "http://cachefly.cachefly.net/100mb.test".into(),
            latency_url: "https://www.cloudflare.com/cdn-cgi/trace".into(),
            min_up: 10,
            max_up: 500,
            min_down: 50,
            max_down: 2000,
            target_accuracy: 10,
            hy_config: PathBuf::from("./hy/config.yaml"),
            hy_binary: PathBuf::from("./hy/hysteria"),
            hy_download_url: "https://download.hysteria.network/app/latest/hysteria-linux-amd64"
                .into(),
        }
    }
}
