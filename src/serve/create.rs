use crate::prelude::*;
use crate::serve::get_server_url;
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use utils::{
    cmd::run_command,
    endpoints::{Endpoint, Method},
    errors::prelude::*,
};

// static IMAGE_REGISTRY: &str = "alelat/wondera";
// static IMAGE_REGISTRY: &str = "ghcr.io/alexlatif/wondera";
// static IMAGE_REGISTRY: &str = "docker.io/alelat/wondera";
static IMAGE_REGISTRY: &str = "h.nodestaking.com/mlx";

lazy_static! {
    static ref REGISTRY_TOKEN: String =
        env::var("GHCR_TOKEN").expect("Environment variable GHCR_TOKEN must be set");
}

#[derive(Deserialize, Debug)]
pub struct TomlConfig {
    service: String,

    stage: String,

    resources: Resources,
}

#[derive(Deserialize, Debug)]
struct Resources {
    cpu_limit: u32,

    gpu_limit: Option<u32>,

    memory_limit: u32,

    concurrent_jobs: u32,

    arch: String,
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

impl ServiceParams {
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

        Ok(ServiceParams {
            input: service_input_params,
            output: service_output_params,
        })
    }
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
pub async fn deploy_service(conf: &TomlConfig) -> RResult<(), AnyErr2> {
    // ensure podman CLI is installed
    // ensure_podman_running().change_context(err2!("Failed to ensure Podman is running"))?;

    let service_id = format!("{}:{}", conf.service, uuid::Uuid::new_v4().to_string());
    let image_uri = format!("{}/{}", IMAGE_REGISTRY, service_id);
    // let image_uri = "h.nodestaking.com/mlx/mnist:fc517390-6af5-4a1d-a00b-b0a459d9990a".to_string();
    // let image_uri = "docker push h.nodestaking.com/mlx/mnist:1".to_string();

    // Build, tag and push new image
    info!(
        "Building, tagging and pushing new image (eta 2-5 mins): {}...",
        image_uri
    );
    match build_tag_and_push_image(&service_id, &image_uri, &conf.resources.arch) {
        Ok(_) => info!("Image {} has been pushed to the registry.", image_uri),
        Err(e) => {
            error!("Failed to build, tag and push image: {}", e);
            return Err(e);
        }
    }

    info!("Reading schema.json...");

    let service_params = {
        let mut file = File::open("schema.json")
            .await
            .expect("Failed to open schema.json");
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .await
            .expect("Failed to read schema.json");

        ServiceParams::from_json(&contents).expect("Failed to build service params")
    };

    info!("Parsing UploadHandlerParams...");
    let cpu_limit = Quantity(conf.resources.cpu_limit.to_string());
    let gpu_limit = Quantity(conf.resources.gpu_limit.unwrap_or(0).to_string());
    let memory_limit = {
        let mem_bytes = conf.resources.memory_limit.to_string();
        let mem_mib = format!("{}Mi", mem_bytes);

        Quantity(mem_mib)
    };

    let replicas = 1;

    let resource_request = ResourceRequest {
        replicas: Some(replicas),
        cpu_limit,
        memory_limit,
        use_gpu: conf.resources.gpu_limit.is_some(),
        gpu_limit,
        concurrent_jobs: conf.resources.concurrent_jobs,
    };

    let upload_handler_params = UploadHandlerParams {
        service_name: conf.service.clone(),
        image_uri: image_uri.clone(),
        resource_request,
        service_schema: service_params,
        env_vars: Some(HashMap::new()),
    };

    debug!("UploadHandlerParams: {:?}", upload_handler_params);

    let endpoint = Endpoint::builder()
        .base_url(&get_server_url().await)
        .endpoint("/upload_service")
        .method(Method::POST)
        .json_body(json!(upload_handler_params))
        .build()
        .unwrap();

    endpoint
        .send()
        .await
        .change_context(err2!("Failed upload_service request"))?;

    info!("Service {} has been deployed successfully.", conf.service);

    Ok(())
}

fn build_tag_and_push_image(_service_id: &str, image_uri: &str, arch: &str) -> RResult<(), AnyErr2> {
    let platform = match arch {
        "amd64" => "linux/amd64",
        "arm64" => "linux/arm64",
        other => panic!("Unsupported architecture: {other}"),
    };

    // run_command("podman", &["system", "prune", "-a", "-f"])
    //     .change_context(err2!("Failed to prune images"))?;

    let mut args = vec![
        "build", "-t", image_uri, ".",
        // "--no-cache"
    ];

    if !platform.is_empty() {
        args.push("--platform");
        args.push(platform);
    }

    print!("Args: {:?}", args);
    run_command("docker", &args).change_context(err2!("Failed to build image"))?;

    login().change_context(err2!("Failed to login to image registry"))?;

    info!("Pushing image to registry... (this may take a few minutes)");

    run_command(
        "docker",
        &[
            "push",
            // "--compression-format=gzip ",
            // "--compression-level=9 ",
            // "--force-compression",
            // "--tls-verify=false",
            image_uri,
        ],
    )
    .change_context(err2!("Failed to push image"))?;

    info!("Removing local image...");

    // run_command("docker", &["rmi", image_uri])
    //     .change_context(err2!("Failed to remove the image"))?;

    Ok(())
}

fn login() -> RResult<(), AnyErr2> {
    let password = "R$G5#XFY&xVMn6IJ";

    let mut cmd = Command::new("docker")
        .arg("login")
        .arg("https://h.nodestaking.com/")
        .arg("--username")
        .arg("wondera")
        .arg("--password-stdin")
        .stdin(Stdio::piped()) // Open a pipe to write to stdin
        .spawn()
        .change_context(err2!("Failed to spawn login command"))?;

    // Write the password to stdin
    if let Some(mut stdin) = cmd.stdin.take() {
        stdin
            .write_all(password.as_bytes())
            .change_context(err2!("Failed to write to stdin"))?;
    }

    // Wait for the command to finish
    let output = cmd
        .wait_with_output()
        .change_context(err2!("Failed to wait for command"))?;

    // Print output for debugging (optional)
    if !output.status.success() {
        eprintln!("Command failed with output: {:?}", output);
    } else {
        println!("Login successful!");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_success() {
        let result = login();
        assert!(result.is_ok(), "Login should succeed");
    }

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

        let result = ServiceParams::from_json(json_data).expect("Failed to build service params");

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
