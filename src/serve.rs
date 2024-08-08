use crate::prelude::*;
// use crate::Method;
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
// use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use utils::errors::{AnyErr, RResult};
use utils::{
    cmd::run_command,
    endpoints::{Endpoint, Method},
    errors::prelude::*,
};

static IMAGE_REGISTRY: &str = "alelat/wondera";
static SERVER_URL: &str = "http://localhost:3000";

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
pub async fn deploy_service(conf: &crate::ServeConfig) -> RResult<(), AnyErr> {
    // ensure podman CLI is installed
    ensure_podman_running()?;

    let service_id = format!("{}-{}", conf.service_name, uuid::Uuid::new_v4().to_string());
    let image_uri = format!("{}:{}", IMAGE_REGISTRY, service_id);

    // Build, tag and push new image
    info!("Building, tagging and pushing new image: {}...", image_uri);
    // match build_tag_and_push_image(&service_id, &image_uri) {
    //     Ok(_) => info!("Image {} has been pushed to the registry.", image_uri),
    //     Err(e) => error!("Failed to push image: {}", e),
    // }

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
        .map_err(|_| err!(AnyErr, "Failed to build Service Schema"))?;

    info!("Parsing UploadHandlerParams...");
    let cpu_limit = Quantity(conf.cpu_limit.as_deref().unwrap_or("0").to_string());
    let gpu_limit = Quantity(conf.gpu_limit.as_deref().unwrap_or("0").to_string());
    let memory_limit = Quantity(conf.memory_limit.clone());

    let resource_request = ResourceRequest {
        replicas: Some(conf.concurrent_jobs as u32),
        cpu_limit,
        memory_limit,
        use_gpu: conf.gpu_limit.is_some(),
        gpu_limit,
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

    // endpoint.send().await.change_context(AnyErr)?;

    Ok(())
}

fn start_podman_machine() -> RResult<(), AnyErr> {
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

fn check_podman_connection() -> RResult<(), AnyErr> {
    let output = Command::new("podman")
        .args(&["system", "connection", "list"])
        .output()
        .change_context(AnyErr)?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.contains("default") {
        info!("Podman connection verified.");
        return Ok(());
    } else {
        return Err(err!(AnyErr, "Podman connection not found."));
    }
}

fn ensure_podman_running() -> RResult<(), AnyErr> {
    if run_command("podman", &["--version"]).is_ok() {
        info!("Podman is already installed.");
    } else {
        if cfg!(target_os = "linux") {
            info!("Installing Podman on Linux...");
            run_command("sudo", &["apt-get", "update"])?;
            run_command("sudo", &["apt-get", "-y", "install", "podman"])?;
        } else if cfg!(target_os = "macos") {
            info!("Installing Podman on macOS...");
            run_command("brew", &["install", "podman"])?;
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

fn build_tag_and_push_image(service_id: &str, image_uri: &str) -> RResult<(), AnyErr> {
    run_command("podman", &["build", "-t", service_id, "."])?;
    run_command("podman", &["tag", service_id, image_uri])?;
    // TODO tmp login of podman
    run_command("podman", &["push", image_uri])?;

    Ok(())
}

// fn build_dynamic_struct(json: &Value) -> HashMap<String, Value> {
//     let mut result = HashMap::new();

//     if let Value::Object(map) = json {
//         for (key, value) in map {
//             result.insert(key.clone(), value.clone());
//         }
//     }

//     result
// }

fn build_service_params_from_json(
    contents: &str,
) -> Result<ServiceParams, Box<dyn std::error::Error>> {
    let json: Value = serde_json::from_str(&contents).expect("Failed to parse JSON");

    let input = json.get("input").ok_or("Missing input field")?;
    let output = json.get("output").ok_or("Missing output field")?;

    let service_input_params = ServiceInputParams {
        path: input
            .get("path")
            .map(|v| serde_json::from_value(v.clone()).unwrap_or_default()),
        query: input
            .get("query")
            .map(|v| serde_json::from_value(v.clone()).unwrap_or_default()),
        body: input
            .get("body")
            .map(|v| serde_json::from_value(v.clone()).unwrap_or_default()),
    };

    let service_output_params: HashMap<String, Param> =
        serde_json::from_value(output.clone()).unwrap_or_default();

    Ok(ServiceParams {
        input: service_input_params,
        output: service_output_params,
    })
}

// fn parse_quantity(quantity: &str) -> RResult<Quantity, AnyErr> {
//     Ok(Quantity(quantity.to_string()))
// }

// is valid service - toml check, docker, build test, run test
// let docker = Docker::connect_with_local_defaults()?;

// // Build image
// let build_opts = BuildImageOptions {
//     // dockerfile: "Dockerfile".to_string(),
//     t: image_uri.clone(),
//     ..Default::default()
// };

// let mut build_stream = docker.build_image(build_opts, None, None);
// while let Some(build_result) = build_stream.next().await {
//     match build_result {
//         Ok(output) => debug!("{:?}", output),
//         Err(e) => error!("Build error: {:?}", e),
//     }
// }

// let tag_options = TagImageOptions {
//     repo: "alelat/wondera",
//     tag: "latest",
// };
// docker
//     .tag_image("alelat/wondera:service_a", Some(tag_options))
//     .await
//     .unwrap();

// // Push image
// let auth_config = AuthConfig {
//     username: Some("your_dockerhub_username".to_string()),
//     password: Some("your_dockerhub_password".to_string()),
//     ..Default::default()
// };
// let push_options = PushImageOptions {
//     tag: "latest",
//     ..Default::default()
// };
// let mut push_stream = docker.push_image("alelat/wondera", Some(push_options), None);
// while let Some(build_result) = push_stream.next().await {
//     match build_result {
//         Ok(output) => debug!("{:?}", output),
//         Err(e) => error!("Push error: {:?}", e),
//     }
// }

// push_stream
//     .for_each(|push_info| async {
//         match push_info {
//             Ok(PushImageInfo { .. }) => println!("Pushed successfully"),
//             Ok(PushImageInfo::Error { message, .. }) => eprintln!("Push error: {}", message),
//             _ => {}
//         }
//     })
//     .await;
// let mut push_stream = docker.push_image(&image_uri, None, None);
// while let Some(push_result) = push_stream.next().await {
//     match push_result {
//         Ok(output) => debug!("{:?}", output),
//         Err(e) => error!("Push error: {:?}", e),
//     }
// }
