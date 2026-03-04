use std::fs::{self, File};
use std::path::Path;
use std::process::Command;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use regex::Regex;
use reqwest::blocking::Client;

use crate::config::Tunable;

pub fn ensure_binary(cfg: &Tunable, log_tx: &Sender<String>) -> Result<()> {
    if cfg.hy_binary.exists() {
        log_tx.send("Hysteria2 二进制已存在，跳过下载".into()).ok();
        return Ok(());
    }

    log_tx.send("未找到 Hysteria2 二进制文件！".into()).ok();
    log_tx
        .send(format!("路径: {}", cfg.hy_binary.display()))
        .ok();
    log_tx.send(format!("URL: {}", cfg.hy_download_url)).ok();
    log_tx.send("开始下载...".into()).ok();

    if let Some(parent) = cfg.hy_binary.parent() {
        fs::create_dir_all(parent)?;
        log_tx.send("目录已创建".into()).ok();
    }

    let client = Client::builder().build()?;
    log_tx.send("连接服务器...".into()).ok();

    let resp = client
        .get(&cfg.hy_download_url)
        .send()
        .context("下载失败")?;

    if !resp.status().is_success() {
        return Err(anyhow!("下载失败，HTTP状态码: {}", resp.status()));
    }

    log_tx.send(format!("响应: {}", resp.status())).ok();
    let bytes = resp.bytes().context("读取响应失败")?;
    log_tx
        .send(format!(
            "大小: {:.1} MB",
            bytes.len() as f64 / 1024.0 / 1024.0
        ))
        .ok();

    let mut file = File::create(&cfg.hy_binary)?;
    std::io::copy(&mut bytes.as_ref(), &mut file)?;
    log_tx.send("文件已保存".into()).ok();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perm = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&cfg.hy_binary, perm)?;
        log_tx.send("设置执行权限".into()).ok();
    }

    log_tx.send("下载完成！".into()).ok();
    Ok(())
}

pub fn parse_socks_port(config_path: &Path) -> Result<u16> {
    let content = fs::read_to_string(config_path)?;
    let re = Regex::new(r"socks5:\s*\n\s*listen:\s*\S+:(\d+)").unwrap();
    if let Some(caps) = re.captures(&content) {
        if let Some(m) = caps.get(1) {
            return m.as_str().parse::<u16>().map_err(|e| anyhow!(e));
        }
    }
    Ok(1080)
}

pub fn restart_hysteria(cfg: &Tunable, log_tx: &Sender<String>) -> Result<()> {
    let bin = cfg.hy_binary.to_string_lossy().to_string();
    let _ = Command::new("pkill").arg("-f").arg(&bin).status();
    let status = Command::new(&bin)
        .args(["client", "-c", &cfg.hy_config.to_string_lossy()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("启动 hysteria 失败")?;
    let _ = status;
    thread::sleep(Duration::from_secs(1));
    log_tx.send("hysteria2 客户端重启成功".into()).ok();
    Ok(())
}

pub fn measure_speed(cfg: &Tunable) -> Result<f64> {
    let output = Command::new("curl")
        .args([
            "-o",
            "/dev/null",
            "-s",
            "-w",
            "%{speed_download}\n",
            &cfg.test_file_url,
        ])
        .output()
        .context("运行 curl 失败")?;
    if !output.status.success() {
        return Err(anyhow!("curl 测速失败"));
    }
    let s = String::from_utf8_lossy(&output.stdout);
    let bps: f64 = s.trim().parse().unwrap_or(0.0);
    Ok(bps * 8.0 / 1024.0 / 1024.0)
}

pub fn measure_latency(cfg: &Tunable, socks_port: u16) -> Result<f64> {
    let proxy = format!("socks5://127.0.0.1:{}", socks_port);
    let output = Command::new("curl")
        .args([
            "-o",
            "/dev/null",
            "-s",
            "-w",
            "%{time_total}\n",
            "--proxy",
            &proxy,
            &cfg.latency_url,
        ])
        .output()
        .context("运行 curl 失败")?;
    if !output.status.success() {
        return Err(anyhow!("curl 延迟测试失败"));
    }
    let s = String::from_utf8_lossy(&output.stdout);
    Ok(s.trim().parse::<f64>().unwrap_or(0.0) * 1000.0)
}

pub fn patch_bandwidth(cfg: &Tunable, param: &str, val: u32) -> Result<()> {
    let content = fs::read_to_string(&cfg.hy_config)?;
    let replaced = if param == "up" {
        Regex::new(r"(?m)^\s*up:\s*.*$")
            .unwrap()
            .replace(&content, format!("  up: {} Mbps", val))
            .to_string()
    } else {
        Regex::new(r"(?m)^\s*down:\s*.*$")
            .unwrap()
            .replace(&content, format!("  down: {} Mbps", val))
            .to_string()
    };
    fs::write(&cfg.hy_config, replaced)?;
    Ok(())
}

pub fn binary_search(
    cfg: &Tunable,
    param: &str,
    min_val: u32,
    max_val: u32,
    target_accuracy: u32,
    socks_port: u16,
    log_tx: &Sender<String>,
) -> Result<(u32, f64)> {
    let mut lo = min_val;
    let mut hi = max_val;
    let mut best_val = lo;
    let mut best_speed = 0.0;
    let mut iter = 0;

    while hi.saturating_sub(lo) > target_accuracy {
        iter += 1;
        let mid = (lo + hi) / 2;
        log_tx
            .send(format!(
                "[{}] 第{}次，范围[{lo},{hi}]，测试 {mid} Mbps",
                param, iter
            ))
            .ok();

        patch_bandwidth(cfg, param, mid)?;
        restart_hysteria(cfg, log_tx)?;

        let speed = measure_speed(cfg)?;
        log_tx.send(format!("速度: {:.2} Mbps", speed)).ok();
        if speed > best_speed {
            best_speed = speed;
            best_val = mid;
            log_tx.send("发现更优配置".into()).ok();
        }

        if speed >= best_speed * 0.95 {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    log_tx
        .send(format!(
            "{} 优化完成: {} Mbps (速度 {:.2} Mbps)",
            param, best_val, best_speed
        ))
        .ok();

    patch_bandwidth(cfg, param, best_val)?;
    restart_hysteria(cfg, log_tx)?;
    let latency = measure_latency(cfg, socks_port).unwrap_or(0.0);
    log_tx
        .send(format!("{} 最终延迟 {:.0} ms", param, latency))
        .ok();
    Ok((best_val, best_speed))
}

pub fn run_tuning(cfg: Tunable, log_tx: Sender<String>) {
    let res = (|| -> Result<()> {
        ensure_binary(&cfg, &log_tx)?;
        let socks = parse_socks_port(&cfg.hy_config)?;

        log_tx.send("开始第一阶段（固定下行优化上行）".into()).ok();
        patch_bandwidth(&cfg, "down", cfg.min_down)?;
        let (best_up, _) = binary_search(
            &cfg,
            "up",
            cfg.min_up,
            cfg.max_up,
            cfg.target_accuracy,
            socks,
            &log_tx,
        )?;

        log_tx.send("开始第二阶段（固定上行优化下行）".into()).ok();
        patch_bandwidth(&cfg, "up", best_up)?;
        let (best_down, _) = binary_search(
            &cfg,
            "down",
            cfg.min_down,
            cfg.max_down,
            cfg.target_accuracy,
            socks,
            &log_tx,
        )?;

        log_tx
            .send(format!(
                "最佳：上行 {} Mbps，下行 {} Mbps",
                best_up, best_down
            ))
            .ok();

        restart_hysteria(&cfg, &log_tx)?;
        let speed = measure_speed(&cfg).unwrap_or(0.0);
        let latency = measure_latency(&cfg, socks).unwrap_or(0.0);
        log_tx.send("===== 调优完成 =====".into()).ok();
        log_tx
            .send(format!(
                "最佳参数：上行 {} Mbps，下行 {} Mbps",
                best_up, best_down
            ))
            .ok();
        log_tx.send(format!("最终速度：{:.2} Mbps", speed)).ok();
        log_tx.send(format!("最终延迟：{:.0} ms", latency)).ok();
        Ok(())
    })();

    if let Err(e) = res {
        let _ = log_tx.send(format!("错误: {e}"));
    }
}
