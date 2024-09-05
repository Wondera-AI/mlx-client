use crate::serve::get_server_url;
use std::collections::HashMap;
use utils::endpoints::{Endpoint, Method};
use utils::prelude::*;

#[tokio::main]
pub async fn delete_service(
    service_name: &str,
    service_version: Option<u32>,
) -> RResult<(), AnyErr2> {
    let mut endpoint_builder = Endpoint::builder()
        .base_url(&get_server_url().await)
        .endpoint(&format!("/delete_service/{}", service_name))
        .method(Method::POST);

    let mut query = HashMap::new();
    if let Some(version) = service_version {
        query.insert("service_version".to_string(), version.to_string());
    }
    endpoint_builder = endpoint_builder.query_params(query);
    let endpoint = endpoint_builder.build().unwrap();

    endpoint
        .send()
        .await
        .change_context(err2!("Failed delete_service request"))?;

    Ok(())
}
