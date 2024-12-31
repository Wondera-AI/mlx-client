use crate::{command, define_error, CommandError};
use error_stack::Result;
use std::collections::HashMap;
use tracing::info;

define_error! {
    pub enum DeployError : CommandError {
        ImageBuild("Image build failed"),
        Upload("Service upload failed"),
        // Config("Configuration validation failed"),
        // Validation("Environment validation failed"),
    }
}

// command! {
//     #[desc = "Deploy a service to the MLX platform"]
//     DeployCommand<DeployError, ()> {
//         #[desc = "Run as Docker proxy mode instead of building image"]
//         proxy: bool = false,

//         #[desc = "Docker image to deploy (required in proxy mode)"]
//         image: Option<String>,

//         #[desc = "Service name for deployment"]
//         name: Option<String>,

//         #[desc = "Environment variables as JSON string (e.g. '{\"KEY\":\"VALUE\"}')"]
//         env: Option<String>,

//         #[desc = "Number of GPUs to request"]
//         gpu_requests: Option<u32>,

//         #[desc = "CPU cores to request (can be fractional)"]
//         cpu_requests: Option<f32>,

//         #[desc = "Memory in MB to request"]
//         mem_requests: Option<u32>,

//         #[desc = "Override default internal port"]
//         internal_port: Option<i32>,

//         #[desc = "Node selector labels (format: key1=value1,key2=value2)"]
//         node_selectors: Option<HashMap<String, String>>,
//     } => DeployHandler
// }

// use std::fmt;

// #[derive(Debug)]
// struct ParseError(String);

// impl fmt::Display for ParseError {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         write!(f, "{}", self.0)
//     }
// }

// impl std::error::Error for ParseError {}

// fn parse_key_val_pairs(s: &str) -> Result<(String, String), ParseError> {
//     let mut parts = s.splitn(2, '=');
//     let key = parts
//         .next()
//         .ok_or_else(|| ParseError("Missing key in key=value pair".to_string()))?;
//     let value = parts
//         .next()
//         .ok_or_else(|| ParseError("Missing value in key=value pair".to_string()))?;
//     Ok((key.to_string(), value.to_string()))
// }

// fn parse_hashmap(input: &str) -> Result<HashMap<String, String>, ParseError> {
//     input
//         .split(',')
//         .map(parse_key_val_pairs)
//         .collect::<Result<HashMap<_, _>, _>>()
// }

command! {
    #[desc = "Deploy a service to the MLX platform"]
    DeployCommand<DeployError, ()> {
        #[arg(help = "Run as Docker proxy mode instead of building image")]
        proxy: bool = false,

        #[arg(help = "Docker image to deploy (required in proxy mode)")]
        image: Option<String>,

        #[arg(help = "Service name for deployment")]
        name: Option<String>,

        #[arg(help = "Environment variables as JSON string (e.g. '{\"KEY\":\"VALUE\"}')")]
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
    async fn execute(cmd: DeployCommand) -> Result<(), DeployError> {
        info!("Deploying service: {:?}", cmd);
        Ok(())
    }
}
// let mut config = Self::load_config(&cmd)?;

// if !cmd.proxy {
//     Self::validate_environment()?;
//     let image = Self::build_image(&config).await?;
//     config.set_image(image);
// }

// Self::deploy_to_mlx(&config).await?;
//     Ok(())
// }

// async fn load_config(cmd: &DeployCommand) -> Result<ServiceConfig, DeployError> {
//     if std::path::Path::new(SERVICE_TOML_PATH).exists() {
//         ServiceConfig::from_toml_file(SERVICE_TOML_PATH)
//             .map_err(DeployError::Config)
//     } else {
//         Self::create_proxy_config(cmd)
//     }
// }

// async fn validate_environment() -> Result<(), DeployError> {
//     let required_files = [SCRIPT_PATH, CONFIG_PATH, SERVICE_TOML_PATH];
//     for file in required_files {
//         if !std::path::Path::new(file).exists() {
//             return Err(DeployError::Validation)?;
//         }
//     }
//     Ok(())
// }

// async fn build_image(config: &ServiceConfig) -> Result<String, DeployError> {
//     let image_uri = format!("{}/{}", IMAGE_REGISTRY, uuid::Uuid::new_v4());

//     tokio::process::Command::new("docker")
//         .args(["build", "-t", &image_uri, "."])
//         .output()
//         .await
//         .map_err(|_| DeployError::ImageBuild)?;

//     Ok(image_uri)
// }

// async fn deploy_to_mlx(config: &ServiceConfig) -> Result<(), DeployError> {
//     reqwest::Client::new()
//         .post(&format!("{}/upload_service", get_server_url().await))
//         .json(config)
//         .send()
//         .await
//         .map_err(|_| DeployError::Upload)?;

//     Ok(())
// }
// }
