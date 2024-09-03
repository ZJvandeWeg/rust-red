# EdgeLink：Rust 开发的 Node-RED 兼容运行时引擎
[![GitHub Actions](https://github.com/oldrev/edgelink/workflows/CICD/badge.svg)](https://github.com/oldrev/edgelink/actions) [![Releases](https://img.shields.io/github/release/oldrev/edgelink.svg)](https://github.com/oldrev/edgelink/releases)

![Node-RED Rust Backend](assets/banner.jpg)

[English](README.md) | 简体中文

### 概述

EdgeLink 是一个以 Rust<sub>†</sub> 为底层语言开发的 [Node-RED](https://nodered.org/) 后端运行时引擎，旨在为 Node-RED 设计的 `flows.json` 流程提供高效的执行环境。EdgeLink 的设计聚焦于提高性能和降低内存消耗，使其能够顺利落地在 CPU 和内存资源受限的边缘计算设备中，从而实现从高性能桌面 PC 到边缘设备的全场景覆盖。

通过在高性能的桌面 PC 上复盘和测试工作流，用户可以将 EdgeLink 与 `flows.json` 工作流文件快速部署到资源有限的边缘计算设备中，实现关键路径的价值转化。

### 特性

![Memory Usage](assets/memory.png)

- **高性能**: 通过 Rust 语言赋能，EdgeLink 在性能上发力，提供原生代码的执行速度，为复杂工作流的执行提供快速响应的能力。
- **低内存占用**: EdgeLink 采用原生代码生态环境，与 Node-RED 的 NodeJS 平台对标，在内存使用上实现了显著的优化，极大地降低了系统的资源倾斜。测试表明，运行同一个简单的工作流，EdgeLink 仅消耗 Node-RED 10% 的物理内存。
- **可扩展性**: 保持 Node-RED 的中台扩展性，通过插件化机制拉通自定义节点的开发。采用紧凑高效的 QuickJS Javascript 解释器为 `function` 节点的 Javascript 脚本提供支持，实现了从点到面的能力协同。
- **尽量兼容 Node-RED**: 在工作流兼容性上尽力对齐 Node-RED 的现有工作流文件，允许用户复用 Node-RED 的设计器进行工作流的开发和测试。考虑到 Rust 是静态语言，Javascript 是动态语言，完全 100% 兼容存在挑战，但在多数场景中已实现较好的兼容性，确保了开发者心智的无缝过渡。

EdgeLink 的设计和实现逻辑，力求通过精细化的资源管理和高效的执行模式，完善工作流在边缘计算设备上的布局，为用户提供一体化的解决方案。

## 快速开始

### 0. 安装 Node-RED

出于测试本项目的目的，我们首先需要安装 Node-RED 作为流程设计器，并生成 flows.json 文件。请参考 Node-RED 的文档获取安装和使用方法。

在 Node-RED 中完成流程设计后，请确保点击大红色的“Deploy”按钮，以生成 flows.json 文件。默认情况下，该文件位于 ~/.node-red/flows.json。请注意不要使用本项目中尚未实现的 Node-RED 功能。

### 1. 构建

```bash
cargo build -r
```

> **Windows 用户请注意:** 为了成功编译项目用到的 `rquickjs` 库，需要确保 `patch.exe` 程序存在于 `%PATH%` 环境变量中。`patch.exe` 用于为 QuickJS 库打上支持 Windows 的补丁，如果你已经安装了 Git，那 Git 都会附带 `patch.exe`。

### 2. 运行

```bash
cargo run -r
```

或者

```bash
./target/release/edgelinkd
```

在默认情况下，EdgeLink 将会读取 ~/.node-red/flows.json 并执行它。

#### 运行单元测试

```bash
cargo test --all
```

#### 运行集成测试

运行集成测试需要首先安装 Python 3.9+ 和对应的 Pytest 依赖库：

```bash
pip install -U -r ./tests/requirements.txt
```

然后执行以下命令即可：

```bash
cargo build -r
python -B -m pytest tests
```


## 配置

在配置文件中可以调整各种设置，例如端口号、`flows.json` 文件位置等。请参考 [CONFIG.md](docs/CONFIG.md) 获取更多信息。

## 项目状态

**原型阶段**：项目当前处于实验原型阶段，不能保证任何稳定性。

## 贡献

欢迎贡献！请阅读 [CONTRIBUTING.md](.github/CONTRIBUTING.md) 获取更多信息。

如果你想支持本项目的开发，可以考虑请我喝杯啤酒：

[![爱发电支持](assets/aifadian.jpg)](https://afdian.com/a/mingshu)

## 反馈与技术支持

我们欢迎任何反馈！如果你遇到任何技术问题或者 bug，请提交 [issue](https://github.com/edge-link/edgelink.rs/issues)。

### 社交网络聊天群：

QQ 群：(198723197)[http://qm.qq.com/cgi-bin/qm/qr?_wv=1027&k=o3gEbpSHbFB6xjtC1Pm2mu0gZG62JNyr&authKey=D1qG9o0Nm%2FlDM8TQJXjr0aYluQ2TQp52wM9RDbNj83jzOy5OpCbHkwEI96SMMJxd&noverify=0&group_code=198723197]

### 联系作者

- 邮箱：oldrev(at)gmail.com
- QQ：55431671

> 超巨型广告：没事儿时可以承接网站前端开发/管理系统开发/PCB 画板打样/单片机开发/压水晶头/中老年陪聊/工地打灰等软硬件项目。

## 许可证

此项目基于 Apache 2.0 许可证 - 详见 [LICENSE](LICENSE) 文件以获取更多详细信息。

版权所有©李维及其他贡献者，保留所有权利。