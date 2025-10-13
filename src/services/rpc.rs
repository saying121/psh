// Copyright (c) 2023-2024 Optimatist Technology Co., Ltd. All rights reserved.
// DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
//
// This file is part of PSH.
//
// PSH is free software: you can redistribute it and/or modify it under the terms of the GNU Lesser General Public License
// as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
//
// PSH is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even
// the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public License along with Performance Savior Home (PSH). If not,
// see <https://www.gnu.org/licenses/>.

use anyhow::{Result, bail};
use chrono::DateTime;
use chrono::{TimeZone, Utc, offset::LocalResult};
use perf_event_rs::sampling::OverflowBy;
use psh_proto::task::TaskType;
use psh_proto::{
    ExportDataReq, GetTaskReq, HeartbeatReq, TaskDoneReq, Unit,
    psh_service_client::PshServiceClient,
};
use std::time::Duration;
use tokio::time::sleep;
use tonic::Code;
use tonic::{
    Request,
    transport::{Channel, ClientTlsConfig, Endpoint},
};

use crate::{config::RpcConfig, runtime::WasmTask, services::host_info::new_info_req};

#[derive(Clone)]
pub struct RpcClient {
    token: String,
    client: PshServiceClient<Channel>,
    max_retries: u32,
    base_delay: Duration,
}

fn into_req<T>(message: T, token: &str) -> Result<Request<T>> {
    let mut req = Request::new(message);
    req.metadata_mut()
        .insert("authorization", format!("Bearer {}", token).parse()?);
    Ok(req)
}

async fn retry_with_backoff<F, T>(
    max_retries: u32,
    base_delay: Duration,
    mut operation: F,
) -> Result<T, tonic::Status>
where
    F: AsyncFnMut() -> Result<T, tonic::Status>,
{
    let mut attempts = 0;
    loop {
        match operation().await {
            Ok(resp) => break Ok(resp),
            Err(status) => {
                attempts += 1;
                if attempts >= max_retries {
                    tracing::error!("RpcClient max retries reached after {} attempts", attempts);
                    break Err(status);
                }

                let retry_delay = base_delay * (2_u32.pow(attempts - 1));

                if status.code() == Code::Unknown && status.message().contains("transport error") {
                    tracing::warn!(
                        "RpcClient transport error detected (attempt {}/{}), retrying in {:?}...",
                        attempts,
                        max_retries,
                        retry_delay
                    );
                    sleep(retry_delay).await;
                    continue;
                }
                break Err(status);
            }
        }
    }
}

pub enum WhichTask {
    Wasm(WasmTask),
    Profiling(ProfilingTask),
    UploadElf(UploadElfTask),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct UploadElfTask {
    pub id: String,
    pub filename: String,
    pub build_id: Option<String>,
}

pub struct ProfilingTask {
    pub id: Option<String>,
    pub process: perf_event_rs::config::Process,
    pub mmap_pages: u64,
    pub overflow_by: OverflowBy,
    pub stack_depth: Option<u16>,
    pub end_time: DateTime<Utc>,
}

impl RpcClient {
    pub async fn new(config: &RpcConfig, token: String) -> Result<Self> {
        let ep = Endpoint::from_shared(config.addr.clone())?
            // 连接相关设置
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            // TCP 相关设置
            .tcp_keepalive(Some(Duration::from_secs(60)))
            .tcp_nodelay(true)
            // HTTP/2相关设置
            .http2_keep_alive_interval(Duration::from_secs(30))
            .keep_alive_while_idle(true)
            // 并发和限流
            .concurrency_limit(256)
            .rate_limit(5, Duration::from_secs(1))
            // TLS 配置
            .tls_config(ClientTlsConfig::new().with_native_roots())?;

        let client: PshServiceClient<Channel> = PshServiceClient::connect(ep).await?;

        Ok(Self {
            token,
            client,
            max_retries: config.max_retries.unwrap_or(3),
            base_delay: config.base_delay.unwrap_or(Duration::from_secs(1)),
        })
    }

    pub async fn send_host_info(&mut self, instance_id: String) -> Result<()> {
        let req = into_req(new_info_req(instance_id), &self.token)?;
        let resp = self.client.send_host_info(req).await?;
        tracing::trace!("{:?}", resp.get_ref());
        Ok(())
    }

    pub async fn export_data(&mut self, message: ExportDataReq) -> Result<()> {
        let req = into_req(message, &self.token)?;
        self.client.export_data(req).await?;
        Ok(())
    }
    pub async fn heartbeat(&mut self, message: HeartbeatReq) -> Result<()> {
        let token = &self.token;

        retry_with_backoff(self.max_retries, self.base_delay, async || {
            let req = into_req(message.clone(), token)
                .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?;
            self.client.heartbeat(req).await
        })
        .await?;
        Ok(())
    }

    pub async fn get_task(&mut self, instance_id: String) -> Result<Option<WhichTask>> {
        let get_task_req = GetTaskReq { instance_id };
        let token = &self.token;

        let response = retry_with_backoff(self.max_retries, self.base_delay, async || {
            let req = into_req(get_task_req.clone(), token)
                .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?;
            self.client.get_task(req).await
        })
        .await?;
        let Some(task): Option<psh_proto::Task> = response.into_inner().task else {
            return Ok(None);
        };

        let LocalResult::Single(end_time) = Utc.timestamp_millis_opt(task.end_time as _) else {
            bail!("Invalid task end time")
        };
        let Some(task_type) = task.task_type else {
            return Ok(None);
        };

        let task = match task_type {
            TaskType::Profiling(profiling_task) => {
                let Some(process) = profiling_task.process else {
                    return Ok(None);
                };
                let Some(overflow_by) = profiling_task.overflow_by else {
                    return Ok(None);
                };
                let process: perf_event_rs::config::Process = process.into();
                WhichTask::Profiling(ProfilingTask {
                    id: task.id.into(),
                    process,
                    mmap_pages: profiling_task.mmap_pages,
                    overflow_by: overflow_by.into(),
                    stack_depth: profiling_task.stack_depth.map(|v| v as _),
                    end_time,
                })
            }
            TaskType::Wasm(wasm_task) => WhichTask::Wasm(WasmTask {
                id: Some(task.id),
                wasm_component: wasm_task.wasm,
                wasm_component_args: wasm_task.wasm_args,
                end_time,
            }),
            TaskType::UploadElf(upload_elf_task) => WhichTask::UploadElf(UploadElfTask {
                id: task.id,
                filename: upload_elf_task.filename,
                build_id: upload_elf_task.build_id,
            }),
        };

        Ok(Some(task))
    }

    pub async fn task_done(
        &mut self,
        task_id: String,
        status: psh_proto::task_done_req::TaskStatus,
    ) -> Result<()> {
        let req = into_req(
            TaskDoneReq {
                task_id,
                status: status as _,
            },
            &self.token,
        )?;
        self.client.task_done(req).await?;
        Ok(())
    }

    pub async fn new_instance_id(&mut self) -> Result<String> {
        let req = into_req(Unit {}, &self.token)?;
        let resp = self.client.new_instance_id(req).await?;
        Ok(resp.into_inner().instance_id)
    }
}
