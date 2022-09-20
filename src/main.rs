use crate::error::NodeRunnerError;
use crate::node::{DefiNode, DEFI_IMAGE};
use bollard::Docker;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use bitcoin_rpc::RpcResult;
use tokio::io::AsyncWriteExt;
use tokio::signal;
use tracing::info;

mod error;
mod node;

#[tokio::main]
async fn main() -> Result<(), NodeRunnerError> {
    tracing_subscriber::fmt::init();
    let mut defi_node = Arc::new(DefiNode::new(
        DEFI_IMAGE.to_string(),
        Some("defi-node".to_string()),
        "defi-data" //TODO: change to use cli args,
    ));
    let _ = defi_node.start().await?;
    tokio::time::sleep(Duration::from_secs(20)).await;
    let handle = {
        let defi_node = defi_node.clone();
        tokio::spawn(async move {
            let mut block_height = i64::MIN;
            loop {
                if let Ok(blockcount) = defi_node.getblockcount().await {
                    if block_height < blockcount {
                        block_height = blockcount;
                        info!("Block Count {}", block_height);
                    }
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });
    };
    signal::ctrl_c().await.expect("Failed to remove node");
    drop(handle);
    info!("Stopping and Removing node");
    defi_node.remove().await?;
    Ok(())
}
