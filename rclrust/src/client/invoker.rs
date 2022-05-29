use std::{fmt, sync::Arc};

use anyhow::Result;
use futures::channel::mpsc;
use rclrust_msg_types::ServiceT;

use super::{ChannelMessage, Client, RclClient};
use crate::{error::RclRustError, internal::worker::WorkerMessage, rclrust_debug, Logger};

pub trait ClientInvokerBase: fmt::Debug {
    fn handle(&self) -> &RclClient;
    fn invoke(&mut self) -> Result<()>;
}

pub struct ClientInvoker<Srv>
where
    Srv: ServiceT,
{
    handle: Arc<RclClient>,
    tx: Option<mpsc::Sender<WorkerMessage<ChannelMessage<Srv>>>>,
}

impl<Srv> ClientInvoker<Srv>
where
    Srv: ServiceT,
{
    pub fn new_from_target(target: &Client<Srv>) -> Self {
        Self {
            handle: target.clone_handle(),
            tx: Some(target.clone_tx()),
        }
    }

    fn stop(&mut self) {
        self.tx.take();
    }
}

impl<Srv> fmt::Debug for ClientInvoker<Srv>
where
    Srv: ServiceT,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ClientInvoker {{{:?}}}", self.handle)
    }
}

impl<Srv> ClientInvokerBase for ClientInvoker<Srv>
where
    Srv: ServiceT,
{
    fn handle(&self) -> &RclClient {
        &self.handle
    }

    fn invoke(&mut self) -> Result<()> {
        if let Some(ref mut tx) = self.tx {
            let res = match self.handle.take_response::<Srv>() {
                Ok(v) => v,
                Err(e) => {
                    return if let Some(RclRustError::RclClientTakeFailed(_)) =
                        e.downcast_ref::<RclRustError>()
                    {
                        rclrust_debug!(
                            Logger::new("rclrust"),
                            "`rcl_wait()` indicate that response is ready, however which incorrect. I know this happens when I use Cyclone DDS."
                        );
                        Ok(())
                    } else {
                        Err(e)
                    };
                }
            };

            match tx.try_send(WorkerMessage::Message(res)) {
                Ok(_) => (),
                Err(e) if e.is_disconnected() => self.stop(),
                Err(_) => {
                    return Err(RclRustError::MessageQueueIsFull {
                        type_: "Client",
                        name: self.handle.service_name().expect("Client should be valid"),
                    }
                    .into())
                }
            }
        }

        Ok(())
    }
}
