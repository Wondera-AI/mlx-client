use crate::serve::deploy::ServiceSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utils::prelude::*;

const DEFAULT_CPU_REQUEST: f32 = 1.0;
const DEFAULT_GPU_REQUEST: u32 = 0; // 0 or 1
const DEFAULT_MEMORY_REQUEST: u32 = 1; // 1Gi
const DEFAULT_CPU_LIMIT: f32 = 10.0;
const DEFAULT_GPU_LIMIT: u32 = 1;
const DEFAULT_MEMORY_LIMIT: u32 = 80; // 2Gi
const DEFAULT_CONCURRENT_JOBS: u32 = 20;
const DEFAULT_ORCHESTRATOR: &str = "kube-io";
const DEFAULT_ARCH: &str = "arm64";

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ResourceRequest {
    pub cpu_requests: Option<f32>,

    pub gpu_requests: Option<u32>,

    pub memory_requests: Option<u32>,

    cpu_limit: Option<f32>,

    gpu_limit: Option<u32>,

    memory_limit: Option<u32>,

    concurrent_jobs: Option<u32>,

    pub arch: Option<String>,
}

impl ResourceRequest {
    pub fn new(
        cpu_requests: Option<f32>,
        gpu_requests: Option<u32>,
        memory_requests: Option<u32>,
        cpu_limit: Option<f32>,
        gpu_limit: Option<u32>,
        memory_limit: Option<u32>,
        concurrent_jobs: Option<u32>,
        arch: Option<String>,
    ) -> Self {
        Self {
            cpu_requests: Some(cpu_requests.unwrap_or(DEFAULT_CPU_REQUEST)),
            gpu_requests: Some(gpu_requests.unwrap_or(DEFAULT_GPU_REQUEST)),
            memory_requests: Some(memory_requests.unwrap_or(DEFAULT_MEMORY_REQUEST)),
            cpu_limit: Some(cpu_limit.unwrap_or(DEFAULT_CPU_LIMIT)),
            gpu_limit: Some(gpu_limit.unwrap_or(DEFAULT_GPU_LIMIT)),
            memory_limit: Some(memory_limit.unwrap_or(DEFAULT_MEMORY_LIMIT)),
            concurrent_jobs: Some(concurrent_jobs.unwrap_or(DEFAULT_CONCURRENT_JOBS)),
            arch: Some(arch.unwrap_or(DEFAULT_ARCH.to_string())),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ServiceConfig {
    pub service: String,

    pub resources: ResourceRequest,

    env_vars: Option<HashMap<String, String>>,

    orchestrator: String,

    is_proxy: Option<bool>, // default to false if not provided

    pub image_uri: Option<String>,

    pub internal_port: Option<i32>,
}

impl ServiceConfig {
    pub fn new(
        service: String,
        resources: ResourceRequest,
        env_vars: Option<HashMap<String, String>>,
        orchestrator: Option<String>,
        is_proxy: bool,
        image_uri: Option<String>,
        internal_port: Option<i32>,
    ) -> Self {
        Self {
            service,
            resources,
            env_vars,
            orchestrator: orchestrator.unwrap_or(DEFAULT_ORCHESTRATOR.to_string()),
            is_proxy: Some(is_proxy),
            image_uri,
            internal_port,
        }
    }

    pub fn from_toml_file(file_path: &str) -> RResult<Self, AnyErr2> {
        let contents =
            std::fs::read_to_string(file_path).change_context(err2!("Failed to read toml file"))?;

        let mut config: ServiceConfig =
            toml::from_str(&contents).change_context(err2!("Failed to parse toml file"))?;

        if config.resources.cpu_requests.is_none() {
            config.resources.cpu_requests = Some(DEFAULT_CPU_REQUEST);
        }

        if config.resources.gpu_requests.is_none() {
            config.resources.gpu_requests = Some(DEFAULT_GPU_REQUEST);
        }

        if config.resources.memory_requests.is_none() {
            config.resources.memory_requests = Some(DEFAULT_MEMORY_REQUEST);
        }

        if config.resources.cpu_limit.is_none() {
            config.resources.cpu_limit = Some(DEFAULT_CPU_LIMIT);
        }

        if config.resources.gpu_limit.is_none() {
            config.resources.gpu_limit = Some(DEFAULT_GPU_LIMIT);
        }

        if config.resources.memory_limit.is_none() {
            config.resources.memory_limit = Some(DEFAULT_MEMORY_LIMIT);
        }

        if config.resources.concurrent_jobs.is_none() {
            config.resources.concurrent_jobs = Some(DEFAULT_CONCURRENT_JOBS);
        }

        if config.is_proxy.is_none() {
            config.is_proxy = Some(false);
        }

        Ok(config)
    }

    pub fn to_upload_handler_params(&self, service_schema: ServiceSchema) -> UploadHandlerParams {
        UploadHandlerParams {
            service_name: self.service.clone(),

            image_uri: self.image_uri.clone().expect("Image URI not provided"),

            resource_request: self.resources.clone(),

            service_schema: Some(service_schema),

            env_vars: self.env_vars.clone(),

            orchestrator: self.orchestrator.clone(),

            is_proxy: self.is_proxy,

            internal_port: self.internal_port,
        }
    }
}

// TODO contract between client and backend
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UploadHandlerParams {
    pub service_name: String,

    pub image_uri: String,

    pub resource_request: ResourceRequest,

    pub service_schema: Option<ServiceSchema>,

    pub env_vars: Option<HashMap<String, String>>,

    pub orchestrator: String,

    pub is_proxy: Option<bool>,

    pub internal_port: Option<i32>,
}
