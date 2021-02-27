use crate::config::{Config, ReadConfigFile};
use crate::env;
use anyhow::Error;
use async_trait::async_trait;
use meio::prelude::{
    Actor, Context, Eliminated, IdOf, InterruptedBy, LiteTask, StartedBy, System, TaskEliminated,
    TaskError,
};
use rill_provider::{config::ProviderConfig, RillProvider};
use rill_server::EmbeddedNode;
use std::net::SocketAddr;

pub struct RillRate {
    // TODO: Keep node addr here as `Option`
    // and if it's not configured than spawn a standalone server
    // and with for it install the port here and spawn a tracer.
    provider_config: Option<ProviderConfig>,
}

impl RillRate {
    pub fn new(app_name: String) -> Self {
        // TODO: Use MyOnceCell here that will inform that it was set
        rill_provider::config::NAME.offer(app_name.into());
        Self {
            provider_config: None,
        }
    }

    fn spawn_provider(&mut self, ctx: &mut Context<Self>) {
        let config = self.provider_config.take().unwrap_or_default();
        let actor = RillProvider::new(config);
        ctx.spawn_actor(actor, Group::Provider);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Group {
    Tuning,
    Provider,
    EmbeddedNode,
}

impl Actor for RillRate {
    type GroupBy = Group;
}

#[async_trait]
impl StartedBy<System> for RillRate {
    async fn handle(&mut self, ctx: &mut Context<Self>) -> Result<(), Error> {
        ctx.termination_sequence(vec![Group::Tuning, Group::Provider, Group::EmbeddedNode]);

        let config_path = env::config();
        let config_task = ReadConfigFile(config_path);
        ctx.spawn_task(config_task, Group::Tuning);

        Ok(())
    }
}

#[async_trait]
impl InterruptedBy<System> for RillRate {
    async fn handle(&mut self, ctx: &mut Context<Self>) -> Result<(), Error> {
        ctx.shutdown();
        Ok(())
    }
}

#[async_trait]
impl Eliminated<RillProvider> for RillRate {
    async fn handle(
        &mut self,
        _id: IdOf<RillProvider>,
        ctx: &mut Context<Self>,
    ) -> Result<(), Error> {
        ctx.shutdown();
        Ok(())
    }
}

#[async_trait]
impl Eliminated<EmbeddedNode> for RillRate {
    async fn handle(
        &mut self,
        _id: IdOf<EmbeddedNode>,
        ctx: &mut Context<Self>,
    ) -> Result<(), Error> {
        ctx.shutdown();
        Ok(())
    }
}

#[async_trait]
impl TaskEliminated<ReadConfigFile> for RillRate {
    async fn handle(
        &mut self,
        _id: IdOf<ReadConfigFile>,
        result: Result<Option<Config>, TaskError>,
        ctx: &mut Context<Self>,
    ) -> Result<(), Error> {
        let config = result
            .map_err(|err| {
                log::warn!(
                    "Can't read config file. No special configuration parameters applied: {}",
                    err
                );
            })
            .ok()
            .and_then(std::convert::identity)
            .unwrap_or_else(|| {
                log::warn!("Default config will be used.");
                Config::default()
            });

        self.provider_config = config.rillrate;
        let node_specified = self
            .provider_config
            .as_ref()
            .map(ProviderConfig::is_node_specified)
            .unwrap_or(false);
        if node_specified {
            self.spawn_provider(ctx);
        } else {
            // If node wasn't specified than spawn an embedded node and
            // wait for the address to spawn a provider connected to that.
            let actor = EmbeddedNode::new(config.server, config.export);
            ctx.spawn_actor(actor, Group::EmbeddedNode);
            let task = WaitForAddr::new();
            ctx.spawn_task(task, Group::Tuning);
        }

        Ok(())
    }
}

#[async_trait]
impl TaskEliminated<WaitForAddr> for RillRate {
    async fn handle(
        &mut self,
        _id: IdOf<WaitForAddr>,
        res: Result<SocketAddr, TaskError>,
        ctx: &mut Context<Self>,
    ) -> Result<(), Error> {
        match res {
            Ok(addr) => {
                log::info!("Connecting tracer to {}", addr);
                rill_provider::config::NODE.offer(addr.to_string());
                self.spawn_provider(ctx);
                Ok(())
            }
            Err(err) => Err(err.into()),
        }
    }
}

struct WaitForAddr {}

impl WaitForAddr {
    fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl LiteTask for WaitForAddr {
    type Output = SocketAddr;

    async fn interruptable_routine(self) -> Result<Self::Output, Error> {
        let intern_rx = rill_server::INTERN_ADDR
            .lock()
            .await
            .1
            .take()
            .ok_or_else(|| Error::msg("intern address notifier not found"))?;
        intern_rx.await.map_err(Error::from)
    }
}
