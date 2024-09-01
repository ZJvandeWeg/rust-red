# EdgeLink: A Node-RED Compatible Runtime Engine in Rust

![Node-RED Rust Backend](assets/banner.jpg)

English | [简体中文](README.zh-cn.md)

## Overview

This is a Node-RED compatible runtime implemented in Rust<sub>†</sub>, designed to enhance performance and reduce memory footprint. By replacing the original NodeJS backend with this Rust-based implementation, you can achieve better performance and a smaller memory footprint.

In summary, you can first test the workflow on a high-performance desktop PC,
and subsequently deploy EdgeLink along with the `flows.json` workflow file
to an edge computing device that is constrained by limited resources for operational execution.

## Features

- **High Performance**: Leverage the advantages of the Rust language for excellent performance.
- **Low Memory Footprint**: Reduce memory usage compared to the NodeJS backend.
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


## Configuration

Adjust various settings in the configuration file, such as port number, `flows.json` path, etc. Refer to [CONFIG.md](docs/CONFIG.md) for more information.

## Project Status

**Prototype Stage**: The project is currently in the prototype stage and cannot guarantee stable operation.

### Node-RED Features Roadmap:

- [x] Flow
- [x] Sublow
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
            - [ ] Dynamic Target (WIP)
        - [x] Link Out
        - [x] Comment (Ignore automatically)
        - [x] GlobalConfig (WIP)
        - [x] Unknown
        - [x] Junction
    - Function nodes:
        - [x] Function
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


## Known Issues

Please refer to [ISSUES.md](docs/ISSUES.md) for a list of known issues and workarounds.

## Feedback and Support

We welcome your feedback! If you encounter any issues or have suggestions, please open an [issue](https://github.com/edge-link/edgelink.rs/issues).

## License

This project is licensed under the Apache 2.0 License - see the [LICENSE](LICENSE) file for more details.