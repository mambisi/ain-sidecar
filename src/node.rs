use crate::error::NodeRunnerError;
use bitcoin_rpc::{BitcoinRpc, RpcResult};
use bollard::container::{
    AttachContainerOptions, AttachContainerResults, Config, CreateContainerOptions, LogOutput,
    RemoveContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::image::CreateImageOptions;
use bollard::Docker;
use futures_util::stream::{StreamExt, TryStreamExt};
use futures_util::Stream;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use bollard::models::{HostConfig, Mount, MountTypeEnum};
use serde_json::Value;
use tokio::signal;

pub(crate) const DEFI_IMAGE: &str = "defi/defichain:latest";

pub struct LogListener {
    inner: Pin<Box<dyn Stream<Item = Result<LogOutput, bollard::errors::Error>> + Send>>,
}

impl LogListener {
    pub async fn next(&mut self) -> Option<Result<LogOutput, NodeRunnerError>> {
        if let res = self.inner.next().await {
            return res.map(|res| res.map_err(|e| e.into()));
        }
        None
    }
}

pub struct DefiNode {
    image: String,
    cname: String,
    host_datadir_path : PathBuf,
}

impl DefiNode {
    pub fn new<P : AsRef<Path>>(image: String, cname: Option<String>, datadir : P ) -> Self {
        Self {
            image,
            cname: cname.unwrap_or("defi-node".to_string()),
            host_datadir_path : PathBuf::from(datadir.as_ref())
        }
    }

    pub async fn start(&self) -> Result<LogListener, NodeRunnerError> {
        let docker = Docker::connect_with_socket_defaults()?;

        docker
            .create_image(
                Some(CreateImageOptions {
                    from_image: self.image.as_str(),
                    ..Default::default()
                }),
                None,
                None,
            )
            .try_collect::<Vec<_>>()
            .await?;
        let host_config = HostConfig {
            mounts : Some(vec![
                Mount {
                    target: Some("/data".to_string()),
                    source: Some(self.host_datadir_path.as_path().to_string_lossy().to_string()),
                    typ: Some(MountTypeEnum::BIND),
                    ..Default::default()
                }

            ]),
            ..Default::default()
        };

        let config = Config {
            image: Some(self.image.as_str()),
            attach_stdout: Some(true),
            host_config : Some(host_config),
            cmd: Some(vec!["defid"]),
            ..Default::default()
        };
        let options = Some(CreateContainerOptions {
            name: self.cname.as_str(),
        });
        docker.create_container(options, config).await?;
        docker.start_container::<String>(&self.cname, None).await?;

        let attach = docker
            .attach_container(
                &self.cname,
                Some(AttachContainerOptions::<String> {
                    stdout: Some(true),
                    stderr: Some(true),
                    stream: Some(true),
                    ..Default::default()
                }),
            )
            .await?;
        Ok(LogListener {
            inner: attach.output,
        })
    }

    pub async fn stop(&self) -> Result<(), NodeRunnerError> {
        let docker = Docker::connect_with_socket_defaults()?;
        docker.stop_container(&self.cname, None).await?;
        Ok(())
    }

    pub async fn getblockcount(&self) -> Result<i64, NodeRunnerError> {
        let docker = Docker::connect_with_socket_defaults()?;
        let exec = docker
            .create_exec(
                self.cname.as_str(),
                CreateExecOptions {
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    cmd: Some(vec!["defi-cli", "getblockcount"]),
                    ..Default::default()
                },
            )
            .await?
            .id;

        let mut output = if let StartExecResults::Attached { mut output, .. } =
            docker.start_exec(&exec, None).await?
        {
            output
        } else {
           return Err(NodeRunnerError::FailedToExec)
        };
        let out = output.next().await.ok_or(NodeRunnerError::FailedToExec)??;
        let value : Value = serde_json::from_str(&out.to_string())?;
        let blockcount = value.as_i64()
            .ok_or(NodeRunnerError::ParseError)?;
        Ok(blockcount)
    }

    // pub async fn rollback(&self, block_height : i64) -> Result<i64, NodeRunnerError> {
    //     let docker = Docker::connect_with_socket_defaults()?;
    //     let exec = docker
    //         .create_exec(
    //             self.cname.as_str(),
    //             CreateExecOptions {
    //                 attach_stdout: Some(true),
    //                 attach_stderr: Some(true),
    //                 cmd: Some(vec!["defi-cli", "getblockcount"]),
    //                 ..Default::default()
    //             },
    //         )
    //         .await?
    //         .id;
    //
    //     let mut output = if let StartExecResults::Attached { mut output, .. } =
    //         docker.start_exec(&exec, None).await?
    //     {
    //         output
    //     } else {
    //        return Err(NodeRunnerError::FailedToExec)
    //     };
    //     let out = output.next().await.ok_or(NodeRunnerError::FailedToExec)??;
    //     let value : Value = serde_json::from_str(&out.to_string())?;
    //     let blockcount = value.as_i64()
    //         .ok_or(NodeRunnerError::ParseError)?;
    //     Ok(blockcount)
    // }

    pub async fn remove(&self) -> Result<(), NodeRunnerError> {
        let docker = Docker::connect_with_socket_defaults()?;
        docker
            .remove_container(
                &self.cname,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await?;
        Ok(())
    }
}
