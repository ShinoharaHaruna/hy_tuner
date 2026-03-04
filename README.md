# hy_tuner

一个用于 **hysteria2 client** 的参数调优小工具。它会在给定测速/延迟探针的前提下，通过二分搜索自动调整 `up/down` 带宽配置，并输出过程日志，便于你快速找到更合适的参数。

## 特性

- **TUI 交互**：参数面板 + 日志面板 + 帮助面板
- **自动确保二进制**：未检测到本地 hysteria2 二进制时，会按配置 URL 自动下载并赋予执行权限
- **自动重启客户端**：每次调整参数后自动重启 hysteria2 client
- **二分搜索调参**：分别优化 `up` 与 `down`，在可控精度内快速收敛
- **日志可读性**：对错误/成功/速度/延迟等关键日志做了颜色区分（普通日志为 Gray，避免浅色终端下不可见）

## 目录结构

- `src/tuner.rs`
  - 下载/确保 hysteria2 二进制
  - patch 配置、重启 client
  - 测速/测延迟、二分搜索逻辑
- `src/ui/`
  - `main.rs`：TUI 主循环
  - `app.rs`：应用状态与事件处理
  - `layout.rs`：界面布局/渲染
  - `input.rs`：输入编辑/参数调整
  - `style.rs`：日志配色
  - `types.rs`：UI 类型定义

## 配置说明

程序的参数来自 `Tunable`（见 `src/config.rs`），默认值如下（摘录概念，具体以代码为准）：

- `hy_config`：hysteria2 配置文件路径，默认 `./hy/config.yaml`
- `hy_binary`：hysteria2 二进制路径，默认 `./hy/hysteria`
- `hy_download_url`：hysteria2 下载地址（Linux amd64）
- `test_file_url`：测速文件 URL（用于 `curl` 下载测速）
- `latency_url`：延迟探针 URL（通过 socks5 代理请求测延迟）
- `min_up/max_up`：上行搜索范围（Mbps）
- `min_down/max_down`：下行搜索范围（Mbps）
- `target_accuracy`：搜索精度（Mbps）

### 关于 hysteria2 配置文件

- 程序会从 `hy_config` 中解析 socks5 监听端口（若解析失败默认 `1080`）。
- 程序会在调参过程中修改配置中的 `up/down` 字段，并反复重启 hysteria2 client。

## 使用方式

首先原地拷贝 `hy/config.yaml.template` 为 `hy/config.yaml`，并填写其中的 `server`、`auth`。`bandwidth` 字段不需要修改，程序会自动调整。

启动程序后，你可以在界面内完成所有操作：

- 参数面板：选择/调整各参数
- 日志面板：观察调优过程与下载/重启等状态
- 帮助面板：快捷键说明（界面底部）

快捷键（也会在界面底部显示）：

- `Tab`：切换焦点（参数/日志）
- `↑↓`：参数面板选择项 / 日志面板滚动
- `←→`：调整数值类参数
- `e`：编辑当前选中的参数（手动输入）
- `s`：开始调优
- `q`：退出

## 依赖与环境假设

- **hysteria2 client**：程序会在 `hy_binary` 不存在时下载。
- **curl**：测速与延迟测试依赖系统 `curl`。
- **pkill**：用于停止旧的 hysteria2 进程。
- 网络访问：需要能够访问 `hy_download_url`、`test_file_url`、`latency_url`。
