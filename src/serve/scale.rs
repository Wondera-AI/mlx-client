use crate::serve::SERVER_URL;
use clap::Args;
use serde_json::json;
use utils::endpoints::{Endpoint, Method};
use utils::prelude::*;

#[derive(Args, Clone)]
pub struct ScaleServiceConf {
    #[arg(help = "Name of the service")]
    service_name: String,

    #[arg(help = "Version of the service")]
    service_version: String,

    #[arg(long, help = "Replicas requested")]
    replicas: Option<u32>,

    #[arg(long, help = "CPU cores or milicores limit requested")]
    cpu_limit: Option<String>,

    #[arg(long, help = "GPU cores or milicores requested")]
    gpu_limit: Option<String>,

    #[arg(long, help = "Memory limit requested")]
    memory_limit: Option<String>,

    #[arg(long, help = "Number of concurrent jobs available per Service")]
    concurrent_jobs: Option<u32>,
}

#[tokio::main]
pub async fn scale_service(conf: &ScaleServiceConf) -> RResult<(), AnyErr2> {
    let mut endpoint_builder = Endpoint::builder()
        .base_url(SERVER_URL)
        .endpoint(&format!(
            "/scale_service/{}/{}",
            conf.service_name, conf.service_version
        ))
        .method(Method::POST);

    let body = json!({
        "replicas": conf.replicas,
        "cpu_limit": conf.cpu_limit,
        "gpu_limit": conf.gpu_limit,
        "memory_limit": conf.memory_limit,
        "concurrent_jobs": conf.concurrent_jobs,
    });
    let endpoint = endpoint_builder.json_body(body).build().unwrap();
    // let endpoint = endpoint_builder.build().unwrap();

    endpoint
        .send()
        .await
        .change_context(err2!("Failed delete_service request"))?;

    Ok(())
}
