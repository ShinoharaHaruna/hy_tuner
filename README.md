# hy_tuner

> **English** | [中文](README_CN.md)

A parameter tuning utility designed for the **hysteria2 client**. Given a speed test and latency probe, it automatically adjusts `up/down` bandwidth configurations using a binary search algorithm. It outputs real-time logs to help you quickly find the optimal parameters for your network environment.

## Features

- **TUI (Terminal User Interface)**: Includes a parameter panel, log panel, and help panel.
- **Automatic Binary Management**: If the local hysteria2 binary is not detected, it automatically downloads it from a configured URL and grants execution permissions.
- **Automatic Client Restart**: Restarts the hysteria2 client automatically after each parameter adjustment.
- **Binary Search Tuning**: Separately optimizes `up` and `down` bandwidth to converge quickly within a controllable accuracy range.
- **Readable Logs**: Color-coded logs for errors, successes, speed, and latency (general logs are set to Gray to remain visible on light-themed terminals).

## Directory Structure

- `src/tuner.rs`
- Download/ensure hysteria2 binary.
- Patch configurations and restart client.
- Speed test/latency test and binary search logic.

- `src/ui/`
- `main.rs`: TUI main loop.
- `app.rs`: Application state and event handling.
- `layout.rs`: UI layout and rendering.
- `input.rs`: Input editing and parameter adjustment.
- `style.rs`: Log color schemes.
- `types.rs`: UI type definitions.

## Configuration

The program parameters are derived from `Tunable` (see `src/config.rs`). Default values include:

- `hy_config`: Path to the hysteria2 config file (default: `./hy/config.yaml`).
- `hy_binary`: Path to the hysteria2 binary (default: `./hy/hysteria`).
- `hy_download_url`: Download URL for hysteria2 (Linux amd64).
- `test_file_url`: URL for speed testing (used via `curl` downloads).
- `latency_url`: Latency probe URL (requested via socks5 proxy).
- `min_up/max_up`: Upload search range (Mbps).
- `min_down/max_down`: Download search range (Mbps).
- `target_accuracy`: Search precision/step size (Mbps).

### About the hysteria2 Config File

- The program parses the socks5 listening port from `hy_config` (defaults to `1080` if parsing fails).
- The program modifies the `up/down` fields in the config file during the tuning process and repeatedly restarts the hysteria2 client.

## Usage

First, copy `hy/config.yaml.template` to `hy/config.yaml` and fill in the `server` and `auth` fields. You do **not** need to manually edit the `bandwidth` field; the program will handle it automatically.

Once the program is started, you can perform all operations within the interface:

- **Parameter Panel**: Select and adjust various parameters.
- **Log Panel**: Monitor the tuning process, download status, and restart events.
- **Help Panel**: Shortcut key instructions (at the bottom of the interface).

### Hotkeys

- `Tab`: Switch focus (Parameters/Logs).
- `↑↓`: Select items in the Parameter panel / Scroll the Log panel.
- `←→`: Adjust numerical parameters.
- `e`: Edit the currently selected parameter (manual input).
- `s`: Start tuning.
- `q`: Quit.

## Dependencies & Environment Assumptions

- **hysteria2 client**: Downloaded automatically if `hy_binary` is missing.
- **curl**: Used for speed and latency tests.
- **pkill**: Used to stop existing hysteria2 processes.
- **Network Access**: Requires access to `hy_download_url`, `test_file_url`, and `latency_url`.
