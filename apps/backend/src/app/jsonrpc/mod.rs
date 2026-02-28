pub mod error;

use jsonrpsee::core::async_trait;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use std::net::SocketAddr;
use tokio::net::ToSocketAddrs;
// use jsonrpsee::PendingSubscriptionSink;
// use jsonrpsee::core::SubscriptionResult;
// use tokio::sync::broadcast;
// use tokio_stream::StreamExt;
// use tokio_stream::wrappers::BroadcastStream;

use jsonrpsee::server::Server;
use jsonrpsee::ws_client::RpcServiceBuilder;

use crate::app::AppState;
use crate::error::Result;

#[rpc(server)]
pub trait Rpc {
    #[method(name = "ping")]
    async fn ping(&self) -> RpcResult<String>;
}

#[derive(Debug)]
pub struct RpcServerImpl {
    pub app_state: AppState,
}

#[async_trait]
impl RpcServer for RpcServerImpl {
    async fn ping(&self) -> RpcResult<String> {
        Ok("pong".to_string())
    }

    // async fn subscribe_stream(&self, pending: PendingSubscriptionSink) -> SubscriptionResult {
    //     let rx = self.coordinator.subscribe_stream();
    //     Self::handle_subscription("subscribe_stream", pending, rx, |x| {
    //         Base64ImFrame::try_from(x)
    //             .tap_err(|e| tracing::error!("Failed to encode frame: {}", e))
    //             .ok()
    //     })
    //     .await
    // }
}

impl RpcServerImpl {
    // async fn handle_subscription<T, R, F>(
    //     name: &'static str,
    //     pending: PendingSubscriptionSink,
    //     rx: broadcast::Receiver<T>,
    //     process_fn: F,
    // ) -> SubscriptionResult
    // where
    //     T: Clone + Send + 'static,
    //     R: Clone + Send + serde::Serialize + 'static,
    //     F: FnMut(T) -> Option<R> + Send + 'static,
    // {
    //     let sink = pending.accept().await?;
    //     let mut stream = BroadcastStream::new(rx)
    //         .filter_map(|x| x.ok())
    //         .filter_map(process_fn);
    //     tracing::debug!("[{}] New subscription: {}", name, sink.connection_id().0);
    //     tokio::spawn(async move {
    //         loop {
    //             tokio::select! {
    //                 Some(msg) = stream.next() => {
    //                     if sink.send(serde_json::value::to_raw_value(&msg).unwrap()).await.is_err() {
    //                         break;
    //                     }
    //                 }
    //                 _ = sink.closed() => break,
    //             }
    //         }
    //         tracing::debug!("[{}] Subscription closed: {}", name, sink.connection_id().0);
    //     });
    //     Ok(())
    // }
}

pub async fn start(app_state: AppState, addr: impl ToSocketAddrs) -> Result<SocketAddr> {
    let rpc_middleware = RpcServiceBuilder::new().rpc_logger(1024);
    let server = Server::builder()
        .set_rpc_middleware(rpc_middleware)
        .build(addr)
        .await?;
    let addr = server.local_addr()?;
    let handle = server.start(RpcServerImpl { app_state }.into_rpc());
    tokio::spawn(handle.stopped());
    Ok(addr)
}
