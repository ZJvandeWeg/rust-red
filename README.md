# EdgeLink: A Node-RED Compatible Run-time Engine in Rust
[![GitHub Actions](https://github.com/oldrev/edgelink/workflows/CICD/badge.svg)](https://github.com/oldrev/edgelink/actions) [![Releases](https://img.shields.io/github/release/oldrev/edgelink.svg)](https://github.com/oldrev/edgelink/releases)

![Node-RED Rust Backend](assets/banner.jpg)

English | [简体中文](README.zh-cn.md)

## Overview

This is a Node-RED compatible run-time engine implemented in Rust<sub>†</sub>. By replacing the original NodeJS backend with this Rust-based implementation, you can achieve better performance and a smaller memory footprint.

In summary, you can first test the workflow on a normal desktop PC,
and subsequently deploy EdgeLink along with the `flows.json` workflow file
to an edge computing device that is constrained by limited resources for operational execution.

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

> **Note for Windows Users:** Windows users should ensure that the `patch.exe` program is available in the `%PATH%` environment variable to successfully compile the project using `rquickjs`. This utility is required to apply patches to the QuickJS library for Windows compatibility. If Git is already installed, it will include `patch.exe`.

### 2. Run

```bash
cargo run -r
```

Or:

```bash
./target/release/edgelinkd
```

By default, EdgeLink will read `~/.node-red/flows.json` and execute it.

#### Run Unit Tests

```bash
cargo test --all
```

#### Run Integration Tests

Running integration tests requires first installing Python 3.9+ and the corresponding Pytest dependencies:

```bash
pip install -U -r ./tests/requirements.txt
```

Then execute the following command:

```bash
cargo build -r
python -B -m pytest tests
```

## Configuration

Adjust various settings in the configuration file, such as port number, `flows.json` path, etc. Refer to [CONFIG.md](docs/CONFIG.md) for more information.

## Project Status

**Prototype Stage**: The project is currently in the prototype stage and cannot guarantee stable operation.

### Node-RED Features Roadmap:

- [x] Flow
- [x] Sub-flow
- [x] Group
- [ ] Environment Variables (WIP)
- [ ] Context
- [ ] RED.util (WIP)
    - [x] `RED.util.cloneMessage()`
    - [x] `RED.util.generateId()`
- [ ] Plug-in subsystem (WIP)

### The Current Status of Nodes:

- Core nodes:
    - Common nodes:
        - [x] Inject
        - [x] Debug (WIP)
        - [x] Complete
        - [ ] Catch
        - [ ] Status
        - [x] Link In
        - [x] Link Call
            - [x] Static Target
            - [x] Dynamic Target
        - [x] Link Out
        - [x] Comment (Ignore automatically)
        - [x] GlobalConfig (WIP)
        - [x] Unknown
        - [x] Junction
    - Function nodes:
        - [x] Function (WIP)
            - [x] Basic functions
            - [ ] `node` object
            - [ ] `context` object
            - [ ] `flow` object
            - [ ] `global` object
            - [x] `RED` object
            - [ ] `env` object
        - [ ] Switch
        - [ ] Change
        - [x] Range
        - [ ] Template
        - [ ] Delay
        - [ ] Trigger
        - [ ] Exec
        - [x] Filter
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

Check out our [roadmap](ROADMAP.md) to get a glimpse of the upcoming features and milestones.

## Contribution

Contributions are welcome! Please read [CONTRIBUTING.md](.github/CONTRIBUTING.md) for more details.

If you want to support the development of this project, you could consider buying me a beer.

[![Support via PayPal.me](assets/paypal_button.svg)](https://www.paypal.me/oldrev)


## Known Issues

Please refer to [ISSUES.md](docs/ISSUES.md) for a list of known issues and workarounds.

## Feedback and Support

We welcome your feedback! If you encounter any issues or have suggestions, please open an [issue](https://github.com/edge-link/edgelink.rs/issues).

## License

This project is licensed under the Apache 2.0 License - see the [LICENSE](LICENSE) file for more details.