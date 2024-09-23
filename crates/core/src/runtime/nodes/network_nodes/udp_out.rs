use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;

use base64::prelude::*;
use serde::Deserialize;

use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;
use edgelink_macro::*;

#[derive(Debug)]
enum UdpMulticast {
    No,
    Board,
    Multi,
}

impl<'de> Deserialize<'de> for UdpMulticast {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "false" => Ok(UdpMulticast::No),
            "board" => Ok(UdpMulticast::Board),
            "multi" => Ok(UdpMulticast::Multi),
            _ => Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&s),
                &"expected 'false' or 'board' or 'multi'",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum UdpIpV {
    #[serde(rename = "udp4")]
    V4,

    #[serde(rename = "udp6")]
    V6,
}

#[derive(Debug)]
#[flow_node("udp out")]
struct UdpOutNode {
    base: FlowNode,
    config: UdpOutNodeConfig,
}

impl UdpOutNode {
    fn build(_flow: &Flow, state: FlowNode, _config: &RedFlowNodeConfig) -> crate::Result<Box<dyn FlowNodeBehavior>> {
        let udp_config = UdpOutNodeConfig::deserialize(&_config.rest)?;

        let node = UdpOutNode { base: state, config: udp_config };
        Ok(Box::new(node))
    }
}

#[derive(Deserialize, Debug)]
struct UdpOutNodeConfig {
    /// Remote address
    #[serde(deserialize_with = "crate::runtime::model::json::deser::str_to_ipaddr")]
    addr: Option<IpAddr>,

    /// Remote port
    #[serde(deserialize_with = "crate::runtime::model::json::deser::str_to_option_u16")]
    port: Option<u16>,

    /// Local address
    #[serde(deserialize_with = "crate::runtime::model::json::deser::str_to_ipaddr")]
    iface: Option<IpAddr>,

    /// Local port
    #[serde(deserialize_with = "crate::runtime::model::json::deser::str_to_option_u16")]
    outport: Option<u16>,

    ipv: UdpIpV,

    #[serde(default)]
    base64: bool,
    //multicast: UdpMulticast,
}

impl UdpOutNode {
    async fn uow(&self, msg: Arc<RwLock<Msg>>, socket: &UdpSocket) -> crate::Result<()> {
        let msg_guard = msg.read().await;
        if let Some(payload) = msg_guard.get("payload") {
            let remote_addr = std::net::SocketAddr::new(
                self.config.addr.unwrap(), // TODO FIXME
                self.config.port.unwrap(),
            );

            if let Some(bytes) = payload.as_bytes() {
                if self.config.base64 {
                    let b64_str = BASE64_STANDARD.encode(bytes);
                    let bytes = b64_str.as_bytes();
                    socket.send_to(bytes, remote_addr).await?;
                } else {
                    socket.send_to(bytes, remote_addr).await?;
                }
            }
            if let Some(bytes) = payload.to_bytes() {
                socket.send_to(&bytes, remote_addr).await?;
            } else {
                log::warn!("Failed to convert payload into bytes");
            }
        }

        Ok(())
    }
}

#[async_trait]
impl FlowNodeBehavior for UdpOutNode {
    fn get_node(&self) -> &FlowNode {
        &self.base
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        let local_addr: SocketAddr = match self.config.outport {
            Some(port) => SocketAddr::new(self.config.iface.unwrap(), port),
            _ => match self.config.ipv {
                UdpIpV::V4 => "0.0.0.0:0".parse().unwrap(),
                UdpIpV::V6 => "[::]:0".parse().unwrap(),
            },
        };

        match tokio::net::UdpSocket::bind(local_addr).await {
            Ok(socket) => {
                let socket = Arc::new(socket);
                while !stop_token.is_cancelled() {
                    let cloned_socket = socket.clone();

                    let node = self.clone();
                    with_uow(node.as_ref(), stop_token.clone(), |node, msg| async move {
                        node.uow(msg, &cloned_socket).await
                    })
                    .await;
                }
            }

            Err(e) => {
                log::error!("Can not bind local address: {:?}", e);
            }
        }
    }
}
