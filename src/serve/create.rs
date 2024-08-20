use crate::prelude::*;
use crate::serve::SERVER_URL;
use clap::Args;
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use serde_json::error;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::process::Command;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use utils::{
    cmd::run_command,
    endpoints::{Endpoint, Method},
    errors::prelude::*,
};

// static IMAGE_REGISTRY: &str = "alelat/wondera";
static IMAGE_REGISTRY: &str = "ghcr.io/alexlatif/wondera";
lazy_static! {
    static ref REGISTRY_TOKEN: String =
        env::var("GHCR_TOKEN").expect("Environment variable GHCR_TOKEN must be set");
}

#[derive(Args, Clone)]
pub struct DeployServiceConf {
    #[arg(help = "Name of the service")]
    service_name: String,

    #[arg(long, help = "Replicas requested")]
    replicas: Option<u32>,

    #[arg(
        long,
        help = "CPU cores or milicores limit requested",
        required_unless_present = "gpu_limit"
    )]
    cpu_limit: Option<String>,

    // #[arg(long, help = "Use available GPU devices")]
    // use_gpu: Option<bool>,
    #[arg(
        long,
        help = "GPU cores or milicores requested",
        // required_unless_present = "use_gpu"
    )]
    gpu_limit: Option<String>,

    #[arg(long, help = "Memory limit requested")]
    memory_limit: String,

    #[arg(long, help = "Number of concurrent jobs available per Service")]
    concurrent_jobs: u32,
    // #[arg(long, help = "Maximum request rate limit")]
    // max_rate_limit: i8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UploadHandlerParams {
    pub service_name: String,

    pub image_uri: String,

    pub resource_request: ResourceRequest,

    pub service_schema: ServiceParams,

    pub env_vars: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequest {
    replicas: Option<u32>,

    cpu_limit: Quantity, // In cores (e.g., 1 for 1 core)

    memory_limit: Quantity, // In human-readable format (e.g., "512Mi")

    use_gpu: bool,

    gpu_limit: Quantity,

    concurrent_jobs: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceParams {
    pub input: ServiceInputParams,

    pub output: HashMap<String, Param>,
}

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

#[tokio::main]
pub async fn deploy_service(conf: &DeployServiceConf) -> RResult<(), AnyErr2> {
    assert!(
        env::var("GHCR_TOKEN").is_ok(),
        "Environment variable GHCR_TOKEN must be set"
    );
    // ensure podman CLI is installed
    ensure_podman_running().change_context(err2!("Failed to ensure Podman is running"))?;

    let service_id = format!("{}-{}", conf.service_name, uuid::Uuid::new_v4().to_string());
    let image_uri = format!("{}:{}", IMAGE_REGISTRY, service_id);
    // let image_uri = "ghcr.io/alexlatif/wondera:a3-5fa813db-f191-4c55-b462-b4e08fde68f5".to_string();

    // Build, tag and push new image
    info!(
        "Building, tagging and pushing new image (eta 2-5 mins): {}...",
        image_uri
    );
    match build_tag_and_push_image(&service_id, &image_uri) {
        Ok(_) => info!("Image {} has been pushed to the registry.", image_uri),
        Err(e) => {
            error!("Failed to build, tag and push image: {}", e);
            return Err(e);
        }
    }

    info!("Image {} has been pushed to the registry.", image_uri);

    info!("Reading config.json...");

    let mut file = File::open("config.json")
        .await
        .expect("Failed to open config.json");

    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .await
        .expect("Failed to read config.json");

    let service_params = build_service_params_from_json(&contents)
        .change_context(err2!("Failed to build service params"))?;

    info!("Parsing UploadHandlerParams...");
    let cpu_limit = Quantity(conf.cpu_limit.as_deref().unwrap_or("0").to_string());
    let gpu_limit = Quantity(conf.gpu_limit.as_deref().unwrap_or("0").to_string());
    let memory_limit = Quantity(conf.memory_limit.clone());
    let replicas = conf.replicas.unwrap_or(1);

    let resource_request = ResourceRequest {
        replicas: Some(replicas),
        cpu_limit,
        memory_limit,
        use_gpu: conf.gpu_limit.is_some(),
        gpu_limit,
        concurrent_jobs: conf.concurrent_jobs,
    };

    let upload_handler_params = UploadHandlerParams {
        service_name: conf.service_name.clone(),
        image_uri: image_uri.clone(),
        resource_request,
        service_schema: service_params,
        env_vars: Some(HashMap::new()),
    };

    debug!("UploadHandlerParams: {:?}", upload_handler_params);

    let endpoint = Endpoint::builder()
        .base_url(SERVER_URL)
        .endpoint("/upload_service")
        .method(Method::POST)
        .json_body(json!(upload_handler_params))
        .build()
        .unwrap();

    endpoint
        .send()
        .await
        .change_context(err2!("Failed upload_service request"))?;

    info!(
        "Service {} has been deployed successfully.",
        conf.service_name
    );

    Ok(())
}

fn start_podman_machine() -> RResult<(), AnyErr2> {
    info!("Initializing Podman machine...");
    if let Err(e) = run_command("podman", &["machine", "init"]) {
        if e.to_string().contains("VM already exists") {
            info!("Podman machine already exists, skipping initialization.");
        } else {
            debug!("Podman machine already started");
        }
    }

    info!("Starting Podman machine...");
    if let Err(e) = run_command("podman", &["machine", "start"]) {
        if e.to_string().contains("VM already running") {
            info!("Podman machine already running, skipping start.");
        } else {
            debug!("Podman machine already started");
        }
    }

    Ok(())
}

fn check_podman_connection() -> RResult<(), AnyErr2> {
    let output = Command::new("podman")
        .args(&["system", "connection", "list"])
        .output()
        .change_context(err2!("Failed to check Podman connection"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.contains("default") {
        info!("Podman connection verified.");
        return Ok(());
    } else {
        return Err(Report::new(err2!("Podman connection not found")));
    }
}

fn ensure_podman_running() -> RResult<(), AnyErr2> {
    if run_command("podman", &["--version"]).is_ok() {
        info!("Podman is already installed.");
    } else {
        if cfg!(target_os = "linux") {
            info!("Installing Podman on Linux...");
            run_command("sudo", &["apt-get", "update"])
                .change_context(err2!("Failed to update"))?;
            run_command("sudo", &["apt-get", "-y", "install", "podman"])
                .change_context(err2!("Failed to install podman"))?;
        } else if cfg!(target_os = "macos") {
            info!("Installing Podman on macOS...");
            run_command("brew", &["install", "podman"])
                .change_context(err2!("Failed to install"))?;
        } else {
            panic!("Unsupported operating system");
        }
    }

    match check_podman_connection() {
        Ok(_) => {
            start_podman_machine()
            // check_podman_connection()
        }
        Err(_) => {
            start_podman_machine()?;
            check_podman_connection()
        }
    }
}

fn build_tag_and_push_image(service_id: &str, image_uri: &str) -> RResult<(), AnyErr2> {
    run_command("podman", &["build", "-t", service_id, "."])
        .change_context(err2!("Failed to build image"))?;
    run_command("podman", &["tag", service_id, image_uri])
        .change_context(err2!("Failed to tag image"))?;

    run_command(
        "podman",
        &[
            "login",
            "ghcr.io",
            "--username",
            "USERNAME",
            "--password",
            REGISTRY_TOKEN.as_str(),
        ],
    )
    .change_context(err2!("Failed to login to image registry"))?;

    info!("Pushing image to registry... (this may take a few minutes)");

    run_command("podman", &["push", image_uri]).change_context(err2!("Failed to push image"))?;

    Ok(())
}

fn build_service_params_from_json(contents: &str) -> RResult<ServiceParams, AnyErr2> {
    debug!("Contents: {:?}", contents);
    let json: Value =
        serde_json::from_str(&contents).expect("Failed to parse config.json contents");

    debug!("JSON: {:?}", json);

    let input = json
        .get("input")
        .ok_or(Report::new(err2!("Missing input field")))?;

    let output = json
        .get("output")
        .ok_or(Report::new(err2!("Missing output field")))?;

    let convert_required = |required: &str| -> bool {
        match required {
            "True" => true,
            _ => false,
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

                let required_str = required_value.as_str().ok_or(Report::new(err2!(format!(
                    "Invalid required field type: {:?}",
                    p
                ))))?;

                let required_bool = convert_required(required_str);

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

                let required_str = required_value.as_str().ok_or(Report::new(err2!(format!(
                    "Invalid required field type: {:?}",
                    p
                ))))?;

                let required_bool = convert_required(required_str);

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

    Ok(ServiceParams {
        input: service_input_params,
        output: service_output_params,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_service_params_from_json() {
        let json_data = r#"
        {
            "input": {
                "path": [{"name": "required_foo", "dtype": "string", "required": "True"}],
                "query": [{"name": "bar", "dtype": "string", "required": "False"}],
                "body": [
                    {"name": "mtype", "dtype": "string", "required": "True"},
                    {"name": "optional_smoothing", "dtype": "integer", "required": "False"}
                ]
            },
            "output": [
                {"name": "foo", "dtype": "string", "required": "True"},
                {"name": "bar", "dtype": "string", "required": "True"}
            ]
        }
        "#;

        let result =
            build_service_params_from_json(json_data).expect("Failed to build service params");

        assert_eq!(result.input.path.as_ref().unwrap()[0].name, "required_foo");
        assert_eq!(result.input.path.as_ref().unwrap()[0].dtype, "string");
        assert!(result.input.path.as_ref().unwrap()[0].required);

        assert_eq!(result.input.query.as_ref().unwrap()[0].name, "bar");
        assert_eq!(result.input.query.as_ref().unwrap()[0].dtype, "string");
        assert!(!result.input.query.as_ref().unwrap()[0].required);

        assert_eq!(result.input.body.as_ref().unwrap()[0].name, "mtype");
        assert_eq!(result.input.body.as_ref().unwrap()[0].dtype, "string");
        assert!(result.input.body.as_ref().unwrap()[0].required);

        assert_eq!(
            result.input.body.as_ref().unwrap()[1].name,
            "optional_smoothing"
        );
        assert_eq!(result.input.body.as_ref().unwrap()[1].dtype, "integer");
        assert!(!result.input.body.as_ref().unwrap()[1].required);

        assert_eq!(result.output["foo"].name, "foo");
        assert_eq!(result.output["foo"].dtype, "string");
        assert!(result.output["foo"].required);

        assert_eq!(result.output["bar"].name, "bar");
        assert_eq!(result.output["bar"].dtype, "string");
        assert!(result.output["bar"].required);
    }
}
