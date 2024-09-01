use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::define_builtin_flow_node;
use crate::red::eval;
use crate::red::json::RedPropertyTriple;
use crate::red::json::RedPropertyType;
use crate::runtime::flow::Flow;
use crate::runtime::model::*;
use crate::runtime::nodes::*;

#[derive(serde::Deserialize, Debug)]
struct InjectNodeConfig {
    #[serde(default)]
    props: Vec<RedPropertyTriple>,

    #[serde(
        default,
        deserialize_with = "crate::red::json::deser::str_to_option_u64"
    )]
    repeat: Option<u64>,

    #[serde(default)]
    crontab: String,

    #[serde(default)]
    once: bool,

    #[serde(rename = "onceDelay", default)]
    once_delay: Option<f64>,

    #[serde(default)]
    topic: String,

    #[serde(default)]
    payload: String,

    #[serde(rename = "payloadType", default)]
    payload_type: String,
}

#[derive(Debug)]
struct InjectNode {
    state: FlowNodeState,

    repeat: Option<f64>,
    cron: Option<String>,
    once: bool,
    once_delay: Option<f64>,
    props: Vec<RedPropertyTriple>,
}

impl InjectNode {
    fn create(
        _flow: &Flow,
        base_node: FlowNodeState,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        // let inject_node_config = InjectNodeConfig::deserialize(&_config.json)?;

        let json = _config.json.clone();
        let mut props = RedPropertyTriple::collection_from_json_value(
            &json
                .get("props")
                .ok_or(EdgelinkError::BadFlowsJson(
                    "Cannot get the `props` property".to_string(),
                ))
                .cloned()?,
        )?;

        if let Some(payload_type) = json.get("payloadType").and_then(|v| v.as_str()) {
            let payload_value_expr = json.get("payload").unwrap().as_str().unwrap();
            props.retain(|x| x.p != "payload");
            props.push(RedPropertyTriple {
                p: "payload".to_string(),
                vt: RedPropertyType::from(payload_type)?,
                v: payload_value_expr.to_string(),
            });
        }

        let node = InjectNode {
            state: base_node,

            repeat: json
                .get("repeat")
                .and_then(|jv| jv.as_str())
                .and_then(|value| value.parse::<f64>().ok()),

            cron: json.get("crontab").and_then(|v| v.as_str()).and_then(|v| {
                if v.is_empty() {
                    None
                } else {
                    Some(format!("0 {}", v))
                }
            }),

            once: json.get("once").unwrap().as_bool().unwrap(),

            once_delay: json
                .get("onceDelay")
                .and_then(|jv| jv.as_str())
                .and_then(|value| value.parse::<f64>().ok()),

            props,
        };
        Ok(Arc::new(node))
    }

    async fn once_task(&self, stop_token: CancellationToken) -> crate::Result<()> {
        if let Some(once_delay_value) = self.once_delay {
            crate::utils::async_util::delay(
                Duration::from_secs_f64(once_delay_value),
                stop_token.child_token(),
            )
            .await?;
        }

        self.inject_msg(stop_token).await?;
        Ok(())
    }

    async fn cron_task(self: Arc<Self>, stop_token: CancellationToken) -> crate::Result<()> {
        let mut sched = JobScheduler::new().await.unwrap_or_else(|e| {
            log::error!("Failed to create JobScheduler: {}", e);
            panic!("Failed to create JobScheduler")
        });

        let cron_expr = match self.cron.as_ref() {
            Some(expr) => expr.as_ref(),
            None => {
                log::error!("Cron expression is missing");
                return Err("Cron expression is missing".into());
            }
        };

        log::debug!("cron_expr='{}'", cron_expr);

        let cron_job_stop_token = stop_token.child_token();
        let self1 = Arc::clone(&self);

        let cron_job_result = Job::new_async(cron_expr, move |_, _| {
            let self2 = Arc::clone(&self1);
            let job_stop_token = cron_job_stop_token.child_token();
            Box::pin(async move {
                if let Err(e) = self2.inject_msg(job_stop_token).await {
                    log::error!("Failed to inject: {}", e);
                }
            })
        });

        match cron_job_result {
            Ok(checked_job) => {
                sched.add(checked_job).await.unwrap_or_else(|e| {
                    log::error!("Failed to add job: {}", e);
                    panic!("Failed to add job")
                });

                sched.start().await.unwrap_or_else(|e| {
                    log::error!("Failed to start scheduler: {}", e);
                    panic!("Failed to start scheduler")
                });

                stop_token.cancelled().await;

                sched.shutdown().await.unwrap_or_else(|e| {
                    log::error!("Failed to shutdown scheduler: {}", e);
                    panic!("Failed to shutdown scheduler")
                });
            }
            Err(e) => {
                log::error!(
                    "Failed to parse cron: '{}' [node.name='{}']: {}",
                    cron_expr,
                    self.name(),
                    e
                );
                return Err(e.into());
            }
        }

        log::info!("The CRON task has been stopped.");
        Ok(())
    }

    async fn repeat_task(
        &self,
        repeat_interval: f64,
        stop_token: CancellationToken,
    ) -> crate::Result<()> {
        while !stop_token.is_cancelled() {
            crate::utils::async_util::delay(
                Duration::from_secs_f64(repeat_interval),
                stop_token.child_token(),
            )
            .await?;
            self.inject_msg(stop_token.child_token()).await?;
        }
        log::info!("The `repeat` task has been stopped.");
        Ok(())
    }

    async fn inject_msg(&self, stop_token: CancellationToken) -> crate::Result<()> {
        let msg_body: BTreeMap<String, Variant> = self
            .props
            .iter()
            .map(|i| {
                (
                    i.p.to_string(),
                    eval::evaluate_node_property(&i.v, &i.vt, Some(self), None).unwrap(),
                )
            })
            .collect();

        let envelope = Envelope {
            port: 0,
            msg: Msg::new_with_body(self.state.id, msg_body),
        };

        {
            let to_notify = envelope.msg.read().await;
            self.notify_uow_completed(&to_notify, stop_token.clone())
                .await;
        }

        self.fan_out_one(&envelope, stop_token.child_token()).await
    }
}

#[async_trait]
impl FlowNodeBehavior for InjectNode {
    fn state(&self) -> &FlowNodeState {
        &self.state
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        if self.once {
            if let Err(e) = self.once_task(stop_token.child_token()).await {
                log::warn!("The 'once_task' failed: {}", e.to_string());
            }
        }

        if let Some(repeat_interval) = self.repeat {
            if let Err(e) = self
                .repeat_task(repeat_interval, stop_token.child_token())
                .await
            {
                log::warn!("The 'repeat_task' failed: {}", e.to_string());
            }
        } else if self.cron.is_some() {
            if let Err(e) = self.clone().cron_task(stop_token.child_token()).await {
                log::warn!("The CRON task failed: {}", e.to_string());
            }
        } else {
            log::warn!(
                "The inject node [id='{}', name='{}'] has no trigger.",
                self.state.id,
                self.state.name
            );
            stop_token.cancelled().await;
        }
    }
}

define_builtin_flow_node!("inject", InjectNode::create);
