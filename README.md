# EdgeLink: A Node-RED Compatible Run-time Engine in Rust
[![Build Status]][actions]
[![Releases](https://img.shields.io/github/release/oldrev/edgelink.svg)](https://github.com/oldrev/edgelink/releases)

[Build Status]: https://img.shields.io/github/actions/workflow/status/oldrev/edgelink/CICD.yml?branch=master
[actions]: https://github.com/oldrev/edgelink/actions?query=branch%3Amaster

![Node-RED Rust Backend](assets/banner.jpg)

English | [简体中文](README.zh-cn.md)

## Overview

EdgeLink is a [Node-RED](https://nodered.org/) compatible run-time engine implemented in Rust<sub>†</sub>.

This program is designed to execute `flows.json` file that have been designed and exported/deployed using Node-RED, without any editor or other HTML/Web-related functionalities. The purpose of its development is to deploy tested Node-RED flows to devices with limited memory for execution.

Only the "function" node will use the lightweight QuickJS JS interpreter to run their code; all other functionalities are implemented in native Rust code.

## Features

![Memory Usage](assets/memory.png)

- **High Performance**: Leverage the advantages of the Rust language for excellent performance.
- **Low Memory Footprint**: Reduce memory usage compared to the NodeJS backend. Tests indicate that, for running a same simple workflow, the physical memory usage of EdgeLink is only 10% of that of Node-RED.
- **Scalability**: Retain the extensibility of Node-RED, supporting custom nodes.
- **Easy Migration**: Easily replace the existing Node-RED backend with minimal modifications.

## Quick Start

### 0. Install Node-RED

For the purpose of testing this project, we first need to install Node-RED as our flow designer and generate the `flows.json` file. Please refer to the Node-RED documentation for its installation and usage.

After completing the flow design in Node-RED, please ensure that you click the big red "Deploy" button to generate the `flows.json` file. By default, this file is located in `~/.node-red/flows.json`. Be mindful not to use Node-RED features that are not yet implemented in this project.

### 1. Build

```bash
cargo build -r
```

> [!IMPORTANT]
> **Note for Windows Users:**
> Windows users should ensure that the `patch.exe` program is available in the `%PATH%` environment variable to successfully compile the project using `rquickjs`. This utility is required to apply patches to the QuickJS library for Windows compatibility. If Git is already installed, it will include `patch.exe`.
>
> To compile `rquickjs`, which is required by the project, you will need to install Microsoft Visual C++ (MSVC) and the corresponding Windows Software Development Kit (SDK).

The toolchains tested are as follows(see GitHub Actions for details):

* `x86_64-pc-windows-msvc`
* `x86_64-pc-windows-gnu`
* `x86_64-unknown-linux-gnu`
* `aarch64-unknown-linux-gnu`
* `armv7-unknown-linux-gnueabihf`
* `armv7-unknown-linux-gnueabi`

### 2. Run

```bash
cargo run -r
```

Or:

```bash
./target/release/edgelinkd
```

By default, EdgeLink will read `~/.node-red/flows.json` and execute it.

You can use the `--help` command-line argument to view all the supported options for this program:

```bash
./target/release/edgelinkd --help
```

#### Run Unit Tests

```bash
cargo test --all
```

#### Run Integration Tests

Running integration tests requires first installing Python 3.9+ and the corresponding Pytest dependencies:

```bash
pip install -r ./tests/requirements.txt
```

Then execute the following command:

```bash
set PYO3_PYTHON=YOUR_PYTHON_EXECUTABLE_PATH # Windows only
cargo build --all
py.test
```

## Configuration

Adjust various settings in the configuration file, such as port number, `flows.json` path, etc. Refer to [CONFIG.md](docs/CONFIG.md) for more information.

## Project Status

**Pre-Alpha Stage**: The project is currently in the *pre-alpha* stage and cannot guarantee stable operation.

The heavy check mark ( :heavy_check_mark: ) below indicates that this feature has passed the integration test ported from Node-RED.

### Node-RED Features Roadmap:

- [x] :heavy_check_mark: Flow
- [x] :heavy_check_mark: Sub-flow
- [x] Group
- [x] Environment Variables (WIP)
- [ ] Context
    - [x] Memory storage
    - [ ] Local file-system storage
- [ ] RED.util (WIP)
    - [x] `RED.util.cloneMessage()`
    - [x] `RED.util.generateId()`
- [x] Plug-in subsystem[^1]
- [ ] JSONata

[^1]: Rust's Tokio async functions cannot call into dynamic libraries, so currently, we can only use statically linked plugins. I will evaluate the possibility of adding plugins based on WebAssembly (WASM) or JavaScript (JS) in the future.

### The Current Status of Nodes:

- Core nodes:
    - Common nodes:
        - [x] :heavy_check_mark: Console-JSON (For integration tests)
        - [x] :heavy_check_mark: Inject
        - [x] Debug (WIP)
        - [x] :heavy_check_mark: Complete
        - [x] Catch
        - [x] Status
        - [x] :heavy_check_mark: Link In
        - [x] :heavy_check_mark: Link Call
        - [x] :heavy_check_mark: Link Out
        - [x] :heavy_check_mark: Comment (Ignored automatically)
        - [x] GlobalConfig (WIP)
        - [x] :heavy_check_mark: Unknown
        - [x] :heavy_check_mark: Junction
    - Function nodes:
        - [x] Function (WIP)
            - [x] Basic functions
            - [x] `node` object (WIP)
            - [x] `context` object
            - [x] `flow` object
            - [x] `global` object
            - [x] `RED.util` object
            - [x] `env` object
        - [ ] Switch
        - [x] :heavy_check_mark: Change
        - [x] :heavy_check_mark: Range
        - [ ] Template
        - [ ] Delay
        - [ ] Trigger
        - [ ] Exec
        - [x] :heavy_check_mark: Filter (RBE)
    - Network nodes:
        - [ ] MQTT In
        - [ ] MQTT Out
        - [ ] HTTP In
        - [ ] HTTP Response
        - [ ] HTTP Request
        - [ ] WebSocket In
        - [ ] WebSocket Out
        - [ ] TCP In
        - [ ] TCP Out
        - [ ] TCP Request
        - [ ] UDP In
        - [x] UDP Out
            - [x] Unicast
            - [ ] Multicast (WIP)
        - [ ] TLS
        - [ ] HTTP Proxy
    - Sqeuence nodes:
        - [ ] Split
        - [ ] Join
        - [ ] Sort
        - [ ] Batch
    - Parse nodes:
        - [ ] CSV
        - [ ] HTML
        - [ ] JSON
        - [ ] XML
        - [ ] YAML
    - Storage
        - [ ] Write File
        - [ ] Read File
        - [ ] Watch

## Roadmap

Check out our [milestones](https://github.com/oldrev/edgelink/milestones) to get a glimpse of the upcoming features and milestones.

## Contribution

![Alt](https://repobeats.axiom.co/api/embed/cd18a784e88be20d79778703bda8858523c4257e.svg "Repobeats analytics image")

Contributions are always welcome! Please read [CONTRIBUTING.md](.github/CONTRIBUTING.md) for more details.

If you want to support the development of this project, you could consider buying me a beer.

<a href='https://ko-fi.com/O5O2U4W4E' target='_blank'><img height='36' style='border:0px;height:36px;' src='https://storage.ko-fi.com/cdn/kofi3.png?v=3' border='0' alt='Buy Me a Coffee at ko-fi.com' /></a>

[![Support via PayPal.me](assets/paypal_button.svg)](https://www.paypal.me/oldrev)

## Known Issues

Please refer to [ISSUES.md](docs/ISSUES.md) for a list of known issues and workarounds.

## Feedback and Support

We welcome your feedback! If you encounter any issues or have suggestions, please open an [issue](https://github.com/edge-link/edgelink/issues).

E-mail: oldrev(at)gmail.com

## License

This project is licensed under the Apache 2.0 License - see the [LICENSE](LICENSE) file for more details.

Copyright © Li Wei and other contributors. All rights reserved.
