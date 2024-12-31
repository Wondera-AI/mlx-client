use super::docker::build_tag_and_push_image;
use super::service::{ResourceRequest, ServiceConfig, ServiceSchema};
use crate::CommandHandler;
use crate::{command, get_server_url, CommandError};
use error_stack::{Report, Result};
use serde_json::json;
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, error, info};
use utils::endpoints::{Endpoint, Method};

static IMAGE_REGISTRY: &str = "h.nodestaking.com/mlx";
static SERVICE_SCHEMA_PATH: &str = "schema.json";
static SERVICE_TOML_PATH: &str = "mlx.toml";

#[derive(Debug, thiserror::Error)]
pub enum DeployError {
    #[error("Failed to parse configuration file: {0}")]
    ConfigParse(String),

    #[error("Service mlx.toml file is missing")]
    MissingToml,

    #[error("Both image and name must be provided when proxy is enabled")]
    MissingImageOrName,

    #[error("Failed to build and push Docker image: {0}")]
    ImageBuildError(String),

    #[error("Failed to parse service schema: {0}")]
    SchemaParseError(String),

    #[error("Failed to construct endpoint: {0}")]
    EndpointBuilder(String),

    #[error("Failed to send request: {0}")]
    RequestError(String),
}

command! {
    #[desc = "Deploy a service to the MLX platform"]
    DeployCommand {
        #[arg(help = "Run as Docker proxy mode instead of building image")]
        proxy: bool,

        #[arg(help = "Docker image to deploy (required in proxy mode)")]
        image: Option<String>,

        #[arg(help = "Service name for deployment")]
        name: Option<String>,

        #[arg(help = "Environment variables (format: key1=value1,key2=value2)")]
        env: Option<String>,

        #[arg(help = "Number of GPUs to request")]
        gpu_requests: Option<u32>,

        #[arg(help = "CPU cores to request (can be fractional)")]
        cpu_requests: Option<f32>,

        #[arg(help = "Memory in MB to request")]
        mem_requests: Option<u32>,

        #[arg(help = "Override default internal port")]
        internal_port: Option<i32>,

        #[arg(
            help = "Node selector labels (format: key1=value1,key2=value2)",
            value_parser = crate::parse_clap_hashmap()
        )]
        node_selectors: Option<HashMap<String, String>>,
    } => DeployHandler
}

pub struct DeployHandler;

impl DeployHandler {
    async fn execute(cmd: DeployCommand) -> Result<(), Report<DeployError>> {
        info!("Deploying service: {:?}", cmd);

        let conf = if std::path::Path::new(SERVICE_TOML_PATH).exists() {
            info!("Service mlx.toml exists, parsing file...");
            ServiceConfig::from_toml_file(SERVICE_TOML_PATH)
                .change_context(DeployError::ConfigParse)?
        } else {
            info!("Service mlx.toml does not exist");

            if !cmd.proxy {
                return Err(Report::new(DeployError::MissingToml));
            }

            match (cmd.image.as_ref(), cmd.name.as_ref()) {
                (Some(image), Some(name)) => ServiceConfig::new_proxy(
                    name.clone(),
                    build_resource_request(&cmd),
                    image.clone(),
                    cmd.internal_port,
                ),
                _ => return Err(Report::new(DeployError::MissingImageOrName)),
            }
        };

        debug!("ServiceConfig: {:?}", conf);

        if !cmd.proxy {
            let service_id = format!("{}:{}", conf.service, uuid::Uuid::new_v4());
            let image_uri = format!("{}/{}", IMAGE_REGISTRY, service_id);

            info!("Building and pushing image (eta 2-5 mins): {}", image_uri);

            build_tag_and_push_image(
                &service_id,
                &image_uri,
                conf.resources.arch.as_deref().unwrap_or_default(),
            )
            .change_context(DeployError::ImageBuildError)?;

            info!("Image {} pushed successfully", image_uri);
        }

        info!("Building ServiceSchema...");
        let service_schema = if cmd.proxy {
            ServiceSchema::default()
        } else {
            ServiceSchema::from_json_file(SERVICE_SCHEMA_PATH)
                .await
                .change_context(DeployError::SchemaParseError)?
        };

        debug!("ServiceSchema: {:?}", service_schema);

        let upload_params = conf.to_upload_handler_params(service_schema);
        let endpoint = Endpoint::builder()
            .base_url(&get_server_url().await)
            .endpoint("/upload_service")
            .method(Method::POST)
            .json_body(json!(upload_params))
            .build()
            .change_context(DeployError::EndpointBuilder)?;

        endpoint.send().await?;

        info!("Service {} deployed successfully", conf.service);
        Ok(())
    }
}

fn build_resource_request(cmd: &DeployCommand) -> ResourceRequest {
    let mut resources = ResourceRequest::default();
    resources.gpu_requests = cmd.gpu_requests.or(resources.gpu_requests);
    resources.cpu_requests = cmd.cpu_requests.or(resources.cpu_requests);
    resources.memory_requests = cmd.mem_requests.or(resources.memory_requests);

    if let Some(selectors) = &cmd.node_selectors {
        resources
            .node_selectors
            .get_or_insert_with(HashMap::new)
            .extend(selectors.clone());
    }

    resources
}

// #[async_trait::async_trait]
// impl CommandHandler<DeployCommand> for DeployHandler {
//     async fn handle(cmd: DeployCommand) -> error_stack::Result<(), CommandError> {
//         info!("Deploying service: {:?}", cmd);

//         // let selectors = cmd.node_selectors.unwrap_or_default();
//         // info!("Node selectors: {:?}", selectors);

//         // let conf: ServiceConfig = if std::path::Path::new(SERVICE_TOML_PATH).exists() {
//         //     info!("Service mlx.toml exists, parsing file...");
//         //     ServiceConfig::from_toml_file(SERVICE_TOML_PATH)
//         //         .map_err(|e| DeployError::ConfigParse(e.to_string()))?
//         // } else {
//         //     info!("Service mlx.toml does not exist");

//         //     let mut resources = ResourceRequest::default();
//         //     resources.gpu_requests = cmd.gpu_requests.or(resources.gpu_requests);
//         //     resources.cpu_requests = cmd.cpu_requests.or(resources.cpu_requests);
//         //     resources.memory_requests = cmd.mem_requests.or(resources.memory_requests);
//         //     resources
//         //         .node_selectors
//         //         .as_mut()
//         //         .map(|existing| existing.extend(selectors.clone()));

//         //     if !cmd.proxy {
//         //         error!("Service mlx.toml must exist when proxy is not enabled.");
//         //         return Err(error_stack::Report::new(DeployError::MissingToml)
//         //             .change_context(CommandError::ExecutionError));
//         //     }

//         //     if cmd.image.is_none() || cmd.name.is_none() {
//         //         error!("Error: Both image and name must be provided when proxy is enabled.");
//         //         return Err(error_stack::Report::new(DeployError::MissingImageOrName)
//         //             .change_context(CommandError::ExecutionError));
//         //     }

//         //     ServiceConfig::new(
//         //         cmd.name
//         //             .clone()
//         //             .expect("Name must be provided when proxy is enabled."),
//         //         resources,
//         //         None,
//         //         None,
//         //         true,
//         //         Some(
//         //             cmd.image
//         //                 .clone()
//         //                 .expect("Image must be provided when proxy is enabled."),
//         //         ),
//         //         cmd.internal_port,
//         //     )
//         // };

//         // debug!("ServiceConfig: {:?}", conf);

//         // if !cmd.proxy {
//         //     let service_id = format!("{}:{}", conf.service, uuid::Uuid::new_v4().to_string());
//         //     let image_uri = format!("{}/{}", IMAGE_REGISTRY, service_id);
//         //     conf.image_uri = Some(image_uri.clone());
//         //     info!(
//         //         "Building, tagging and pushing new image (eta 2-5 mins): {}...",
//         //         image_uri
//         //     );

//         //     build_tag_and_push_image(
//         //         &service_id,
//         //         &image_uri,
//         //         &conf.resources.arch.as_deref().unwrap_or_default(),
//         //     )
//         //     .map_err(|e| DeployError::ImageBuildError(e.to_string()))
//         //     .change_context(CommandError::ExecutionError)?;

//         //     info!("Image {} has been pushed to the registry.", image_uri);
//         // }

//         // info!("Building ServiceSchema...");
//         // let service_schema: ServiceSchema = if cmd.proxy {
//         //     ServiceSchema::default()
//         // } else {
//         //     ServiceSchema::from_json_file(SERVICE_SCHEMA_PATH)
//         //         .await
//         //         .map_err(|e| DeployError::SchemaParseError(e.to_string()))
//         //         .change_context(CommandError::ExecutionError)?
//         // };
//         // debug!("ServiceSchema: {:?}", service_schema);

//         // info!("Building UploadHandlerParams...");
//         // let upload_handler_params = conf.to_upload_handler_params(service_schema);
//         // debug!("UploadHandlerParams: {:?}", upload_handler_params);

//         // let endpoint = Endpoint::builder()
//         //     .base_url(&get_server_url().await)
//         //     .endpoint("/upload_service")
//         //     .method(Method::POST)
//         //     .json_body(json!(upload_handler_params))
//         //     .build()
//         //     .map_err(|e| DeployError::EndpointBuilder(e.to_string()))
//         //     .change_context(CommandError::ExecutionError)?
//         //     .send()
//         //     .await
//         //     .map_err(|e| DeployError::RequestError(e.to_string()))
//         //     .change_context(CommandError::ExecutionError)?;

//         // info!("Service {} has been deployed successfully.", conf.service);

//         Ok(())
//     }
// }
