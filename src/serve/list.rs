use crate::serve::SERVER_URL;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, CellAlignment, ContentArrangement, Table};
use serde_json::Value;
use std::collections::HashMap;
use utils::endpoints::{Endpoint, Method};
use utils::prelude::*;

#[tokio::main]
pub async fn list_services(service_name: Option<&str>) -> RResult<Value, AnyErr2> {
    let mut endpoint_builder = Endpoint::builder()
        .base_url(SERVER_URL)
        .endpoint("/list_service")
        .method(Method::GET);

    let mut query = HashMap::new();
    if let Some(name) = service_name {
        query.insert("service_name".to_string(), name.to_string());
    }
    endpoint_builder = endpoint_builder.query_params(query);
    let endpoint = endpoint_builder.build().unwrap();

    let response = endpoint
        .send()
        .await
        .change_context(err2!("Failed list_service request"))?;

    let services = response
        .as_array()
        .ok_or_else(|| err2!("Response is not an array"))?;

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_width(180)
        .set_header(vec![
            "Name",
            "Version",
            "CPU Limit",
            "Memory Limit",
            "Replicas",
            "Running",
            "Pod ID",
        ]);

    for service in services {
        let pod_id = service["pod_id"].as_str().unwrap_or("-");
        let name = service["name"].as_str().unwrap_or("-");
        let version = service["version"].as_i64().unwrap_or(0).to_string();
        let cpu_limit = service["resource_request"]["cpu_limit"]
            .as_str()
            .unwrap_or("-");
        let memory_limit = service["resource_request"]["memory_limit"]
            .as_str()
            .unwrap_or("-");
        let replicas = service["resource_request"]["replicas"]
            .as_i64()
            .unwrap_or(0)
            .to_string();
        let running = service["running"].as_bool().unwrap_or(false).to_string();

        table.add_row(vec![
            Cell::new(name),
            Cell::new(version).set_alignment(CellAlignment::Center),
            Cell::new(cpu_limit),
            Cell::new(memory_limit),
            Cell::new(replicas).set_alignment(CellAlignment::Center),
            Cell::new(running).set_alignment(CellAlignment::Center),
            Cell::new(pod_id),
        ]);
    }

    println!("{table}");

    Ok(response)
}
