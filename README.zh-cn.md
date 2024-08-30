# EdgeLink：Rust 开发的 Node-RED 兼容运行时引擎

![Node-RED Rust Backend](assets/banner.jpg)

[English](README.md) | 简体中文

## 概述

EdgeLink 是一个使用 Rust<sub>†</sub> 编写的 Node-RED 后端运行时引擎，用于执行 Node-RED 设计产生的 `flows.json` 流程，旨在提高性能并降低内存占用以利于部署到
CPU 和内存资源紧张的边缘计算设备。

总之，你可以在你性能强大的桌面 PC 上测试工作流，然后将 EdgeLink 和 flows.json 工作流文件部署到资源受限的边缘计算设备中运行。


## 特性

- **低内存占用**: 编译成原生代码，对比起 Node-RED 的 NodeJS 平台，极大降低了内存使用。
- **高性能**: 使用 Rust 开发，提供原生代码的性能优势。
- **可扩展性**: 保留了 Node-RED 的可扩展性，支持插件式的自定义节点。采用紧凑的 QuickJS Javascript 解释器提供 `function` 节点的 Javascript 脚本支持。
- **尽量兼容 Node-RED**: 工作流尽可能兼容现有 Node-RED 工作流文件，可以直接利用 NodeRED 的设计器进行工作流开发和测试。当然由于 Rust 是静态语言，Javascript 是动态语言，难以做到 100% 兼容。

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

## 配置

在配置文件中可以调整各种设置，例如端口号、`flows.json` 文件位置等。请参考 [CONFIG.md](docs/CONFIG.md) 获取更多信息。

## 项目状态

**原型阶段**：项目当前处于实验原型阶段，不能保证任何稳定性。

## 贡献

欢迎贡献！请阅读 [CONTRIBUTING.md](.github/CONTRIBUTING.md) 获取更多信息。


## 反馈与技术支持

我们欢迎任何反馈！如果你遇到任何技术问题或者 bug，请提交 [issue](https://github.com/edge-link/edgelink.rs/issues)。

## 许可证

此项目基于 Apache 2.0 许可证 - 详见 [LICENSE](LICENSE) 文件以获取更多详细信息。