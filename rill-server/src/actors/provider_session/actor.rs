use super::link;
use crate::actors::client_session::PROVIDER;
use crate::actors::exporter::ExporterLinkForProvider;
use crate::actors::router::Router;
use anyhow::Error;
use async_trait::async_trait;
use meio::{
    ActionHandler, Actor, Context, IdOf, InteractionHandler, InterruptedBy, StartedBy,
    TaskEliminated, TaskError,
};
use meio_connect::{
    server::{WsHandler, WsProcessor},
    TermReason, WsIncoming,
};
use rill_protocol::client::{ClientReqId, ClientResponse};
use rill_protocol::provider::{
    EntryId, Path, ProviderProtocol, ProviderReqId, ProviderToServer, RillEvent, ServerToProvider,
};
use rill_protocol::transport::{DirectId, Direction, Envelope, WideEnvelope};
use std::collections::HashMap;
use typed_slab::TypedSlab;

pub struct ProviderSession {
    /*
    tracer: EntryTracer,
    tracer_record: Option<ProviderRecord>,
    */
    handler: WsHandler<ProviderProtocol>,
    registered: Option<EntryId>,
    exporter: ExporterLinkForProvider,
    counter: usize,
    // TODO: Replace to `TypedSlab`
    paths: HashMap<DirectId<ProviderProtocol>, Path>,

    directions: TypedSlab<ProviderReqId, (link::ClientSender, ClientReqId)>,
}

impl ProviderSession {
    pub fn new(handler: WsHandler<ProviderProtocol>, exporter: ExporterLinkForProvider) -> Self {
        Self {
            handler,
            registered: None,
            exporter,
            counter: 0,
            paths: HashMap::new(),

            directions: TypedSlab::new(),
        }
    }

    fn send_request(&mut self, direct_id: ProviderReqId, data: ServerToProvider) {
        let envelope = Envelope { direct_id, data };
        log::trace!("Sending request to the server: {:?}", envelope);
        self.handler.send(envelope);
    }

    async fn graceful_shutdown(&mut self, ctx: &mut Context<Self>) {
        if self.registered.take().is_some() {
            self.exporter.session_detached().await.ok();
        }
        ctx.shutdown();
    }
}

#[async_trait]
impl Actor for ProviderSession {
    type GroupBy = ();
}

#[async_trait]
impl StartedBy<Router> for ProviderSession {
    async fn handle(&mut self, ctx: &mut Context<Self>) -> Result<(), Error> {
        let worker = self.handler.worker(ctx.address().clone());
        ctx.spawn_task(worker, ());
        Ok(())
    }
}

#[async_trait]
impl InterruptedBy<Router> for ProviderSession {
    async fn handle(&mut self, ctx: &mut Context<Self>) -> Result<(), Error> {
        self.graceful_shutdown(ctx).await;
        Ok(())
    }
}

#[async_trait]
impl TaskEliminated<WsProcessor<ProviderProtocol, Self>> for ProviderSession {
    async fn handle(
        &mut self,
        _id: IdOf<WsProcessor<ProviderProtocol, Self>>,
        _result: Result<TermReason, TaskError>,
        ctx: &mut Context<Self>,
    ) -> Result<(), Error> {
        self.graceful_shutdown(ctx).await;
        Ok(())
    }
}

impl ProviderSession {
    async fn distribute_data(
        &mut self,
        direction: Direction<ProviderProtocol>,
        event: RillEvent,
    ) -> Result<(), Error> {
        if let Direction::Direct(direct_id) = direction {
            let path = self.paths.get(&direct_id);
            if let Some(path) = path.cloned() {
                if let Err(err) = self.exporter.data_received(path, event).await {
                    log::error!("Can't send data item to the exporter: {}", err);
                }
            } else {
                log::error!(
                    "Unknown direction {:?} of the incoming data {:?}",
                    direct_id,
                    event
                );
            }
        } else {
            log::error!(
                "Not supported direction {:?} of the incoming data {:?}",
                direction,
                event
            );
        }
        Ok(())
    }
}

#[async_trait]
impl ActionHandler<WsIncoming<WideEnvelope<ProviderProtocol, ProviderToServer>>>
    for ProviderSession
{
    async fn handle(
        &mut self,
        msg: WsIncoming<WideEnvelope<ProviderProtocol, ProviderToServer>>,
        ctx: &mut Context<Self>,
    ) -> Result<(), Error> {
        log::trace!("Provider incoming message: {:?}", msg);
        match msg.0.data {
            ProviderToServer::Data { batch } => {
                let ids = msg.0.direction.into_vec();
                // TODO: Send whole batch
                let resp = ClientResponse::Data(batch);
                for direct_id in &ids {
                    if let Some((sender, direct_id)) = self.directions.get(*direct_id) {
                        let envelope = WideEnvelope {
                            direction: (*direct_id).into(),
                            data: resp.clone(),
                        };
                        sender.send(envelope);
                    }
                }
                /*
                for event in batch {
                    self.distribute_data(msg.0.direction.clone(), event).await?;
                }
                */
            }
            ProviderToServer::BeginStream { snapshot } => {
                /*
                // It's important to forward the snapshot, because it
                // a stream doesn't generate data too often, but the provider
                // can keep it than we can have the current value in exporters.
                for event in snapshot {
                    log::trace!("Processing snapshot event: {:?}", event);
                    self.distribute_data(msg.0.direction.clone(), event).await?;
                }
                */
            }
            ProviderToServer::EndStream => {}
            ProviderToServer::Declare { entry_id } => {
                ctx.not_terminating()?;
                self.exporter
                    .session_attached(entry_id.clone(), ctx.address().link())
                    .await?;
                self.registered = Some(entry_id);
                *PROVIDER.lock().await = Some(ctx.address().link());
                let msg = ServerToProvider::Describe { active: true };
                self.send_request(0.into(), msg);
            }
            ProviderToServer::Description { list } => {
                log::trace!("Paths available: {:?}", list);
                for description in list {
                    if let Err(err) = self.exporter.path_declared(description).await {
                        log::error!("Can't notify exporter about a new path: {}", err);
                    }
                }
            }
            other => {
                log::warn!("Message {:?} not supported yet.", other);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl InteractionHandler<link::SubscribeToPath> for ProviderSession {
    async fn handle(
        &mut self,
        msg: link::SubscribeToPath,
        _ctx: &mut Context<Self>,
    ) -> Result<ProviderReqId, Error> {
        let direct_id = self.directions.insert((msg.sender, msg.direct_id));

        let request = ServerToProvider::ControlStream {
            path: msg.path,
            active: true,
        };
        self.send_request(direct_id, request);

        Ok(direct_id)
    }
}

#[async_trait]
impl InteractionHandler<link::NewRequest> for ProviderSession {
    async fn handle(
        &mut self,
        msg: link::NewRequest,
        _ctx: &mut Context<Self>,
    ) -> Result<ProviderReqId, Error> {
        self.counter += 1;
        let direct_id = DirectId::from(self.counter);

        if let ServerToProvider::ControlStream {
            ref path,
            active: true,
        } = msg.request
        {
            self.paths.insert(direct_id, path.clone());
        }

        self.send_request(direct_id, msg.request);

        Ok(direct_id)
    }
}

#[async_trait]
impl ActionHandler<link::SubRequest> for ProviderSession {
    async fn handle(
        &mut self,
        msg: link::SubRequest,
        _ctx: &mut Context<Self>,
    ) -> Result<(), Error> {
        let direct_id = msg.direct_id;
        self.paths.remove(&direct_id);
        self.send_request(direct_id, msg.request);
        Ok(())
    }
}
