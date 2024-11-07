use crate::prelude::*;
use crate::serve::deploy::{build_tag_and_push_image, ServiceConfig, ServiceSchema};
use crate::serve::get_server_url;
use crate::SERVICE_SCHEMA_PATH;
use serde_json::json;
use utils::{
    endpoints::{Endpoint, Method},
    errors::prelude::*,
};

static IMAGE_REGISTRY: &str = "h.nodestaking.com/mlx";

pub async fn deploy_service(conf: &mut ServiceConfig, is_proxy: bool) -> RResult<(), AnyErr2> {
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
        ServiceSchema::from_json_file(SERVICE_SCHEMA_PATH)
            .await
            .change_context(err2!("Failed to build service params"))?
    } else {
        ServiceSchema::default()
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
