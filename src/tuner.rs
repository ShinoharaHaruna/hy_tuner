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

// 枚举类型，增强类型安全
#[derive(Clone, Copy, Debug)]
pub enum Direction {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug)]
pub enum SearchPhase {
    Coarse,
    Fine,
}

// 评分算法常量
const LATENCY_THRESHOLD_MS: f64 = 100.0; // 延迟惩罚阈值
const LATENCY_DECAY_MS: f64 = 50.0; // 延迟衰减斜率
const JITTER_CV_THRESHOLD: f64 = 0.2; // 抖动变异系数阈值
const JITTER_DECAY: f64 = 0.1; // 抖动衰减斜率
const BANDWIDTH_PREFERENCE_WEIGHT: f64 = 0.3; // 带宽偏好权重

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

// 上行测速（通过 POST 发送数据测试）
// 使用 dd 生成数据并通过管道传给 curl
pub fn measure_speed_upload(cfg: &Tunable) -> Result<f64> {
    use std::process::{Command, Stdio};

    // 创建子进程管道：dd -> curl
    let dd_process = Command::new("dd")
        .args(["if=/dev/zero", "bs=1M", "count=5", "status=none"]) // status=none 禁用进度输出
        .stdout(Stdio::piped())
        .spawn()
        .context("启动 dd 失败")?;

    let dd_stdout = dd_process
        .stdout
        .ok_or_else(|| anyhow!("无法获取 dd 输出"))?;

    let curl_output = Command::new("curl")
        .args([
            "-o",
            "/dev/null",
            "-s",
            "-w",
            "%{speed_upload}\n",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/octet-stream",
            "--data-binary",
            "@-",
            &cfg.test_file_url,
        ])
        .stdin(Stdio::from(dd_stdout))
        .output()
        .context("curl 上行测速失败")?;

    if !curl_output.status.success() {
        return Err(anyhow!("curl 上行测速失败"));
    }

    let s = String::from_utf8_lossy(&curl_output.stdout);
    let bps: f64 = s
        .trim()
        .parse()
        .map_err(|_| anyhow!("无效的上行速度值: {}", s.trim()))?;
    Ok(bps * 8.0 / 1024.0 / 1024.0)
}

// 多次测量取平均值，同时测量延迟和抖动
pub fn measure_comprehensive(
    cfg: &Tunable,
    socks_port: u16,
    log_tx: &Sender<String>,
    phase: SearchPhase,
    direction: Direction,
) -> Result<(f64, f64, f64)> {
    let (samples, interval) = match phase {
        SearchPhase::Coarse => (2, 500),
        SearchPhase::Fine => (3, 600),
    };

    let mut speeds = Vec::with_capacity(samples);
    let mut latencies = Vec::with_capacity(samples);

    for i in 0..samples {
        let speed = match direction {
            Direction::Up => measure_speed_upload(cfg)?,
            Direction::Down => measure_speed(cfg)?,
        };

        let latency = measure_latency(cfg, socks_port).unwrap_or(0.0);
        speeds.push(speed);
        latencies.push(latency);

        if i < samples - 1 {
            thread::sleep(Duration::from_millis(interval));
        }
    }

    let avg_speed: f64 = speeds.iter().sum::<f64>() / samples as f64;
    let avg_latency: f64 = latencies.iter().sum::<f64>() / samples as f64;
    let speed_std =
        (speeds.iter().map(|&s| (s - avg_speed).powi(2)).sum::<f64>() / samples as f64).sqrt();

    // 输出测量结果
    let label = match direction {
        Direction::Up => "上行",
        Direction::Down => "下行",
    };
    log_tx
        .send(format!(
            "{}{}: {:.1} Mbps, {:.0} ms",
            if matches!(phase, SearchPhase::Coarse) {
                "  "
            } else {
                "    "
            },
            label,
            avg_speed,
            avg_latency
        ))
        .ok();

    Ok((avg_speed, avg_latency, speed_std))
}

// 计算综合评分
// 策略：在延迟合理的前提下，优先选择速度稳定且带宽较低的配置
fn calculate_score(
    bandwidth: u32,
    speed: f64,
    latency: f64,
    speed_std: f64,
    max_bandwidth: u32,
    log_tx: &Sender<String>,
) -> f64 {
    // 延迟惩罚：延迟超过阈值开始惩罚，指数增长
    let latency_penalty = if latency > LATENCY_THRESHOLD_MS {
        1.0 + ((latency - LATENCY_THRESHOLD_MS) / LATENCY_DECAY_MS).powf(2.0)
    } else {
        1.0
    };

    // 抖动惩罚：速度变异系数(CV)超过阈值开始惩罚
    let cv = if speed > 0.0 { speed_std / speed } else { 1.0 };
    let jitter_penalty = if cv > JITTER_CV_THRESHOLD {
        1.0 + ((cv - JITTER_CV_THRESHOLD) / JITTER_DECAY).powf(2.0)
    } else {
        1.0
    };

    // 带宽偏好：偏向较低带宽（避免过度配置）
    let bandwidth_preference =
        1.0 + (bandwidth as f64 / max_bandwidth as f64) * BANDWIDTH_PREFERENCE_WEIGHT;

    // 综合评分：速度 / (延迟惩罚 × 抖动惩罚 × 带宽偏好)
    let score = speed / (latency_penalty * jitter_penalty * bandwidth_preference);

    log_tx
        .send(format!(
            "      评分分析: 延迟惩罚={:.2}, 抖动惩罚={:.2}, 带宽偏好={:.2}, 最终得分={:.2}",
            latency_penalty, jitter_penalty, bandwidth_preference, score
        ))
        .ok();

    score
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

// 测量网络基准带宽（不经过代理）
// 用于快速确定搜索区间的中心点
pub fn measure_baseline_bandwidth(cfg: &Tunable, log_tx: &Sender<String>) -> Result<u32> {
    log_tx
        .send("正在测量网络基准带宽（直连，不经过代理）...".into())
        .ok();

    let mut speeds = Vec::with_capacity(2);

    for i in 0..2 {
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
        let mbps = bps * 8.0 / 1024.0 / 1024.0;
        speeds.push(mbps);
        log_tx
            .send(format!("  基准测试 {}/2: {:.2} Mbps", i + 1, mbps))
            .ok();

        if i < 1 {
            thread::sleep(Duration::from_millis(500));
        }
    }

    let avg_speed = (speeds[0] + speeds[1]) / 2.0;
    let baseline = (avg_speed * 1.2) as u32; // 留20%余量

    log_tx
        .send(format!(
            "基准带宽估算: {:.2} Mbps → 设置为 {} Mbps",
            avg_speed, baseline
        ))
        .ok();

    Ok(baseline)
}

pub fn patch_bandwidth(cfg: &Tunable, param: Direction, val: u32) -> Result<()> {
    let content = fs::read_to_string(&cfg.hy_config)?;
    let replaced = match param {
        Direction::Up => Regex::new(r"(?m)^\s*up:\s*.*$")
            .unwrap()
            .replace(&content, format!("  up: {} Mbps", val))
            .to_string(),
        Direction::Down => Regex::new(r"(?m)^\s*down:\s*.*$")
            .unwrap()
            .replace(&content, format!("  down: {} Mbps", val))
            .to_string(),
    };
    fs::write(&cfg.hy_config, replaced)?;
    Ok(())
}

// 两阶段黄金分割搜索：粗搜索快速定位 + 精搜索准确收敛
// 基于综合评分的优化搜索（智能区间收缩版本）
// 策略：
// 1. 先测网络基准带宽（直连）
// 2. 快速收敛到 [baseline/2, baseline*1.5] 区间
// 3. 在缩小的区间内精确搜索
pub fn optimal_bandwidth_search(
    cfg: &Tunable,
    param: Direction,
    min_val: u32,
    max_val: u32,
    target_accuracy: u32,
    socks_port: u16,
    log_tx: &Sender<String>,
) -> Result<(u32, f64)> {
    const RESPHI: f64 = 2.0 - 1.618_033_988_749_895;

    // === 第一步：测量网络基准带宽 ===
    let baseline = measure_baseline_bandwidth(cfg, log_tx)?;

    // === 第二步：智能收缩搜索区间 ===
    // 原始区间可能是 [50, 2000]，现在缩小到 [baseline/2, baseline*1.5]
    let search_min = (min_val).max(baseline / 2);
    let search_max = (max_val).min((baseline as u64 * 3 / 2) as u32); // 防止 u32 溢出

    // 防止区间过小或异常
    let search_min = search_min.max(min_val);
    let search_max = search_max.max(search_min + target_accuracy * 2);

    log_tx
        .send(format!(
            "[{}] {} 调优",
            if matches!(param, Direction::Up) {
                "up"
            } else {
                "down"
            },
            if matches!(param, Direction::Up) {
                "上行"
            } else {
                "下行"
            }
        ))
        .ok();
    log_tx
        .send(format!(
            "  搜索区间: 原始[{}, {}] → 收缩后[{}, {}] (基准: {} Mbps)",
            min_val, max_val, search_min, search_max, baseline
        ))
        .ok();

    // === 第三步：动态调整搜索精度 ===
    // 根据区间大小调整精度，区间越大精度越粗
    let range_size = search_max - search_min;
    let adjusted_accuracy = if range_size > 1000 {
        target_accuracy * 3 // 大区间用粗精度
    } else if range_size > 500 {
        target_accuracy * 2 // 中区间用中精度
    } else {
        target_accuracy // 小区间用精确精度
    };

    log_tx
        .send(format!(
            "  搜索精度: {} Mbps (区间宽度: {} Mbps)",
            adjusted_accuracy, range_size
        ))
        .ok();

    let mut a = search_min as f64;
    let mut b = search_max as f64;
    let mut iter = 0;
    const MAX_ITER: usize = 12; // 区间已缩小，减少最大迭代次数

    // 初始化两个测试点
    let mut x1 = b - RESPHI * (b - a);
    let mut x2 = a + RESPHI * (b - a);

    // 测试 x1
    log_tx
        .send(format!("  [初始化] 测试点 x1: {} Mbps", x1 as u32))
        .ok();
    patch_bandwidth(cfg, param, x1 as u32)?;
    restart_hysteria(cfg, log_tx)?;
    let (s1, l1, std1) =
        measure_comprehensive(cfg, socks_port, log_tx, SearchPhase::Coarse, param)?;
    let mut score1 = calculate_score(x1 as u32, s1, l1, std1, search_max, log_tx);

    // 测试 x2
    log_tx
        .send(format!("  [初始化] 测试点 x2: {} Mbps", x2 as u32))
        .ok();
    patch_bandwidth(cfg, param, x2 as u32)?;
    restart_hysteria(cfg, log_tx)?;
    let (s2, l2, std2) =
        measure_comprehensive(cfg, socks_port, log_tx, SearchPhase::Coarse, param)?;
    let mut score2 = calculate_score(x2 as u32, s2, l2, std2, search_max, log_tx);

    let mut best_x = if score1 > score2 { x1 } else { x2 };
    let mut best_score = score1.max(score2);
    let (mut best_speed, mut best_latency) = if score1 > score2 { (s1, l1) } else { (s2, l2) };

    log_tx
        .send(format!(
            "  初始最优: {} Mbps (评分: {:.2}, 速度: {:.2} Mbps, 延迟: {:.0} ms)",
            best_x as u32, best_score, best_speed, best_latency
        ))
        .ok();

    // 黄金分割主循环
    while (b - a) > adjusted_accuracy as f64 && iter < MAX_ITER {
        iter += 1;
        log_tx
            .send(format!(
                "  第{}轮: 搜索区间 [{:.0}, {:.0}] (宽度: {:.0})",
                iter,
                a,
                b,
                b - a
            ))
            .ok();

        if score1 > score2 {
            // x1 评分更高，丢弃 [x2, b] 区间
            log_tx
                .send(format!(
                    "    → {} Mbps 评分更优 ({:.2} > {:.2})，丢弃上区间",
                    x1 as u32, score1, score2
                ))
                .ok();

            b = x2;
            x2 = x1;
            score2 = score1;

            if (b - a) > adjusted_accuracy as f64 * 2.0 {
                // 需要新的测试点
                x1 = b - RESPHI * (b - a);
                log_tx
                    .send(format!("    新测试点: {} Mbps", x1 as u32))
                    .ok();

                patch_bandwidth(cfg, param, x1 as u32)?;
                restart_hysteria(cfg, log_tx)?;
                let (s, l, std) =
                    measure_comprehensive(cfg, socks_port, log_tx, SearchPhase::Coarse, param)?;
                score1 = calculate_score(x1 as u32, s, l, std, search_max, log_tx);

                if score1 > best_score {
                    best_score = score1;
                    best_x = x1;
                    best_speed = s;
                    best_latency = l;
                    log_tx
                        .send(format!("    ✓ 发现更优配置: {} Mbps", x1 as u32))
                        .ok();
                }
            }
        } else {
            // x2 评分更高或相当，丢弃 [a, x1] 区间
            log_tx
                .send(format!(
                    "    → {} Mbps 评分更优 ({:.2} >= {:.2})，丢弃下区间",
                    x2 as u32, score2, score1
                ))
                .ok();

            a = x1;
            x1 = x2;
            score1 = score2;

            if (b - a) > adjusted_accuracy as f64 * 2.0 {
                // 需要新的测试点
                x2 = a + RESPHI * (b - a);
                log_tx
                    .send(format!("    新测试点: {} Mbps", x2 as u32))
                    .ok();

                patch_bandwidth(cfg, param, x2 as u32)?;
                restart_hysteria(cfg, log_tx)?;
                let (s, l, std) =
                    measure_comprehensive(cfg, socks_port, log_tx, SearchPhase::Coarse, param)?;
                score2 = calculate_score(x2 as u32, s, l, std, search_max, log_tx);

                if score2 > best_score {
                    best_score = score2;
                    best_x = x2;
                    best_speed = s;
                    best_latency = l;
                    log_tx
                        .send(format!("    ✓ 发现更优配置: {} Mbps", x2 as u32))
                        .ok();
                }
            }
        }
    }

    let best_bw = best_x.round() as u32;

    let param_label = if matches!(param, Direction::Up) {
        "up"
    } else {
        "down"
    };
    log_tx
        .send(format!("[{}] 优化完成: {} Mbps", param_label, best_bw))
        .ok();
    log_tx
        .send(format!(
            "  最终指标: 速度={:.2} Mbps, 延迟={:.0} ms, 评分={:.2}",
            best_speed, best_latency, best_score
        ))
        .ok();

    // 最终验证（使用更精确的测量）
    patch_bandwidth(cfg, param, best_bw)?;
    restart_hysteria(cfg, log_tx)?;
    let (final_speed, final_latency, _) =
        measure_comprehensive(cfg, socks_port, log_tx, SearchPhase::Fine, param)?;

    log_tx
        .send(format!(
            "  验证结果: 速度={:.2} Mbps, 延迟={:.0} ms",
            final_speed, final_latency
        ))
        .ok();

    Ok((best_bw, final_speed))
}

pub fn run_tuning(cfg: Tunable, log_tx: Sender<String>) {
    let res = (|| -> Result<()> {
        ensure_binary(&cfg, &log_tx)?;
        let socks = parse_socks_port(&cfg.hy_config)?;

        log_tx.send("开始第一阶段（固定下行优化上行）".into()).ok();
        patch_bandwidth(&cfg, Direction::Down, cfg.min_down)?;
        let (best_up, _) = optimal_bandwidth_search(
            &cfg,
            Direction::Up,
            cfg.min_up,
            cfg.max_up,
            cfg.target_accuracy,
            socks,
            &log_tx,
        )?;

        log_tx.send("开始第二阶段（固定上行优化下行）".into()).ok();
        patch_bandwidth(&cfg, Direction::Up, best_up)?;
        let (best_down, _) = optimal_bandwidth_search(
            &cfg,
            Direction::Down,
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
        let (final_speed, final_latency, _) =
            measure_comprehensive(&cfg, socks, &log_tx, SearchPhase::Fine, Direction::Down)?;
        let latency = final_latency; // 使用测量的延迟
        log_tx.send("===== 调优完成 =====".into()).ok();
        log_tx
            .send(format!(
                "最佳参数：上行 {} Mbps，下行 {} Mbps",
                best_up, best_down
            ))
            .ok();
        log_tx
            .send(format!("最终速度：{:.2} Mbps", final_speed))
            .ok();
        log_tx.send(format!("最终延迟：{:.0} ms", latency)).ok();
        Ok(())
    })();

    if let Err(e) = res {
        let _ = log_tx.send(format!("错误: {e}"));
    }
}
