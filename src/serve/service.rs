use serde::{Deserialize, Serialize};
use std::{collections::HashMap, process::exit};
use utils::prelude::*;

const DEFAULT_CPU_REQUEST: f32 = 1.0;
const DEFAULT_GPU_REQUEST: u32 = 0; // 0 or 1
const DEFAULT_MEMORY_REQUEST: u32 = 1; // 1Gi
const DEFAULT_CPU_LIMIT: u32 = 4;
const DEFAULT_GPU_LIMIT: u32 = 1;
const DEFAULT_MEMORY_LIMIT: u32 = 100; // 100Gi
const DEFAULT_CONCURRENT_JOBS: u32 = 20;
const DEFAULT_ORCHESTRATOR: &str = "kube";
const DEFAULT_ARCH: &str = "arm64";

fn default_node_target_labels() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("nvidia.com/gpu.present".to_string(), "true".to_string());
    map
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ResourceRequest {
    pub cpu_requests: Option<f32>,

    pub gpu_requests: Option<u32>,

    pub memory_requests: Option<u32>,

    cpu_limit: Option<u32>,

    gpu_limit: Option<u32>,

    memory_limit: Option<u32>,

    concurrent_jobs: Option<u32>,

    pub arch: Option<String>,

    pub node_selectors: Option<HashMap<String, String>>,
}

impl ResourceRequest {
    pub fn new(
        cpu_requests: Option<f32>,
        gpu_requests: Option<u32>,
        memory_requests: Option<u32>,
        cpu_limit: Option<u32>,
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
            node_selectors: None,
        }
    }

    pub fn default() -> Self {
        Self {
            cpu_requests: Some(DEFAULT_CPU_REQUEST),
            gpu_requests: Some(DEFAULT_GPU_REQUEST),
            memory_requests: Some(DEFAULT_MEMORY_REQUEST),
            cpu_limit: Some(DEFAULT_CPU_LIMIT),
            gpu_limit: Some(DEFAULT_GPU_LIMIT),
            memory_limit: Some(DEFAULT_MEMORY_LIMIT),
            concurrent_jobs: Some(DEFAULT_CONCURRENT_JOBS),
            arch: Some(DEFAULT_ARCH.to_string()),
            node_selectors: Some(default_node_target_labels()),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ServiceConfig {
    pub service: String,

    pub resources: ResourceRequest,

    env_vars: Option<HashMap<String, String>>,

    orchestrator: Option<String>, // default to "wondera" if not provided

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
            orchestrator: Some(orchestrator.unwrap_or(DEFAULT_ORCHESTRATOR.to_string())),
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

        if config.resources.arch.is_none() {
            config.resources.arch = Some(DEFAULT_ARCH.to_string());
        }

        if config.resources.node_selectors.is_none() {
            config.resources.node_selectors = Some(default_node_target_labels());
        }

        if config.is_proxy.is_none() {
            config.is_proxy = Some(false);
        }

        if config.orchestrator.is_none() {
            config.orchestrator = Some(DEFAULT_ORCHESTRATOR.to_string());
        }

        if config.image_uri.is_none() {
            error!("Image URI not provided");
            exit(0);
        }

        debug!("ServiceConfig: {:?}", config);

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

    pub orchestrator: Option<String>,

    pub is_proxy: Option<bool>,

    pub internal_port: Option<i32>,
}

use serde_json::Value;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceInputParams {
    pub path: Option<Vec<Param>>,

    pub query: Option<Vec<Param>>,

    pub body: Option<Vec<Param>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Param {
    pub name: String,

    pub dtype: String,

    pub required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceSchema {
    pub input: ServiceInputParams,

    pub output: HashMap<String, Param>,
}

impl ServiceSchema {
    pub fn default() -> Self {
        ServiceSchema {
            input: ServiceInputParams {
                path: None,
                query: None,
                body: None,
            },
            output: HashMap::new(),
        }
    }

    pub async fn from_json_file(file_name: &str) -> RResult<Self, AnyErr2> {
        let mut file = File::open(file_name)
            .await
            .expect("Failed to open schema.json");

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .await
            .expect("Failed to read schema.json");

        ServiceSchema::from_json(&contents)
    }

    pub fn from_json(contents: &str) -> RResult<Self, AnyErr2> {
        debug!("Contents: {:?}", contents);
        let json: Value =
            serde_json::from_str(&contents).expect("Failed to parse schema.json contents");

        debug!("JSON: {:?}", json);

        let input = json
            .get("input")
            .ok_or(Report::new(err2!("Missing input field")))?;

        let output = json
            .get("output")
            .ok_or(Report::new(err2!("Missing output field")))?;

        let convert_required = |required: &Value| -> RResult<bool, AnyErr2> {
            if let Some(required_str) = required.as_str() {
                match required_str {
                    "True" | "true" => Ok(true),
                    "False" | "false" => Ok(false),
                    _ => Err(Report::new(err2!(format!(
                        "Invalid required field string: {:?}",
                        required
                    )))),
                }
            } else if let Some(required_bool) = required.as_bool() {
                Ok(required_bool)
            } else {
                Err(Report::new(err2!(format!(
                    "Invalid required field type: {:?}",
                    required
                ))))
            }
        };

        let convert_params = |params: &Value| -> RResult<Vec<Param>, AnyErr2> {
            debug!("Converting params: {:?}", params);
            let result = params
                .as_array()
                .ok_or(Report::new(err2!(format!(
                    "Expected array, found {:?}",
                    params
                ))))?
                .iter()
                .map(|p| {
                    let mut param_map: serde_json::Map<String, Value> = p
                        .as_object()
                        .ok_or(Report::new(err2!(format!(
                            "Expected object, found {:?}",
                            p
                        ))))?
                        .clone();

                    let required_value =
                        param_map
                            .remove("required")
                            .ok_or(Report::new(err2!(format!(
                                "Missing required field in param: {:?}",
                                p
                            ))))?;
                    let required_bool = convert_required(&required_value)?;
                    param_map.insert("required".to_string(), Value::Bool(required_bool));

                    let param: Param = serde_json::from_value(Value::Object(param_map))
                        .change_context(err2!(format!("Failed to convert param: {:?}", p)))?;

                    Ok(param)
                })
                .collect::<RResult<Vec<Param>, AnyErr2>>();

            debug!("Converted params result: {:?}", result);
            result
        };

        let service_input_params = ServiceInputParams {
            path: input
                .get("path")
                .map_or(Ok(None), |v| convert_params(v).map(Some))?,
            query: input
                .get("query")
                .map_or(Ok(None), |v| convert_params(v).map(Some))?,
            body: input
                .get("body")
                .map_or(Ok(None), |v| convert_params(v).map(Some))?,
        };

        debug!("Service input params: {:?}", service_input_params);

        let convert_output_params = |params: &Value| -> RResult<HashMap<String, Param>, AnyErr2> {
            debug!("Converting output params: {:?}", params);
            let result = params
                .as_array()
                .ok_or(Report::new(err2!(format!(
                    "Expected array, found {:?}",
                    params
                ))))?
                .iter()
                .map(|p| {
                    let mut param_map: serde_json::Map<String, Value> = p
                        .as_object()
                        .ok_or(Report::new(err2!(format!(
                            "Expected object, found {:?}",
                            p
                        ))))?
                        .clone();

                    let required_value =
                        param_map
                            .remove("required")
                            .ok_or(Report::new(err2!(format!(
                                "Missing required field in param: {:?}",
                                p
                            ))))?;

                    let required_bool = convert_required(&required_value)?;
                    param_map.insert("required".to_string(), Value::Bool(required_bool));

                    let param: Param = serde_json::from_value(Value::Object(param_map))
                        .change_context(err2!(format!("Failed to convert param: {:?}", p)))?;

                    Ok((param.name.clone(), param))
                })
                .collect::<RResult<HashMap<String, Param>, AnyErr2>>();

            debug!("Converted output params result: {:?}", result);
            result
        };

        let service_output_params: HashMap<String, Param> = convert_output_params(output)?;

        Ok(ServiceSchema {
            input: service_input_params,
            output: service_output_params,
        })
    }
}
