use crate::prelude::*;
use crate::serve::deploy::{
    build_tag_and_push_image, ResourceRequest, ServiceConfig, ServiceSchema,
};
use crate::serve::get_server_url;
use crate::{SERVICE_SCHEMA_PATH, SERVICE_TOML_PATH};
use serde_json::json;
use utils::{
    endpoints::{Endpoint, Method},
    errors::prelude::*,
};

static IMAGE_REGISTRY: &str = "h.nodestaking.com/mlx";

pub async fn deploy_service(
    is_proxy: bool,
    name: Option<String>,
    image: Option<String>,
    cluster: String,
    env: Option<String>,
    gpu_requests: Option<u32>,
    cpu_requests: Option<f32>,  
    mem_requests: Option<u32>,
    internal_port: Option<i32>,
) -> RResult<(), AnyErr2> {
    let mut conf: ServiceConfig = if std::path::Path::new(SERVICE_TOML_PATH).exists() {
        info!("Service mlx.toml exists, parsing file...");
        ServiceConfig::from_toml_file(SERVICE_TOML_PATH).unwrap()
    } else {
        info!("Service mlx.toml does not exist");
        let resources = ResourceRequest::new(
            cpu_requests,
            gpu_requests,
            mem_requests,
            None,
            None,
            None,
            None,
            None,
        );

        if !is_proxy {
            error!("Service mlx.toml must exist when `proxy` is not enabled.");
            std::process::exit(1);
        }
        if image.is_none() || name.is_none() {
            error!("Error: Both `image` and `name` must be provided when `proxy` is enabled.");
            std::process::exit(1);
        }
        let env_map = env
            .map(|env_str| {
                serde_json::from_str(&env_str).change_context(err2!("Failed to parse env"))
            })
            .transpose()?;

        debug!("Env map: {:?}", env_map);

        ServiceConfig::new(
            name.clone()
                .expect("Name must be provided when `proxy` is enabled."),
            resources,
            env_map,
            Some(cluster),
            is_proxy,
            Some(
                image
                    .clone()
                    .expect("Image must be provided when `proxy` is enabled."),
            ),
            internal_port,
        )
    };

    debug!("ServiceConfig: {:?}", conf);

    if !is_proxy {
        let service_id = format!("{}:{}", conf.service, uuid::Uuid::new_v4().to_string());
        let image_uri = format!("{}/{}", IMAGE_REGISTRY, service_id);
        // let image_uri = "h.nodestaking.com/mlx/mnist:fc517390-6af5-4a1d-a00b-b0a459d9990a".to_string();
        conf.image_uri = Some(image_uri.clone());
        info!(
            "Building, tagging and pushing new image (eta 2-5 mins): {}...",
            image_uri
        );
        match build_tag_and_push_image(
            &service_id,
            &image_uri,
            &conf.resources.arch.as_deref().expect("Failed to set arch"),
        ) {
            Ok(_) => info!("Image {} has been pushed to the registry.", image_uri),
            Err(e) => {
                error!("Failed to build, tag and push image: {}", e);
                return Err(e);
            }
        }
    }

    info!("Building ServiceSchema...");
    let service_schema: ServiceSchema = if is_proxy {
        ServiceSchema::default()
    } else {
        ServiceSchema::from_json_file(SERVICE_SCHEMA_PATH)
            .await
            .change_context(err2!("Failed to build service params"))?
    };
    debug!("ServiceSchema: {:?}", service_schema);

    info!("Building UploadHandlerParams...");
    let upload_handler_params = conf.to_upload_handler_params(service_schema);
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
