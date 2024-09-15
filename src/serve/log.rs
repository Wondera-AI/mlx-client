use crate::serve::get_server_url;
use chrono::DateTime;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, CellAlignment, ContentArrangement, Table};
use serde_json::Value;
use std::collections::HashMap;
use utils::endpoints::{Endpoint, Method};
use utils::prelude::*;

#[tokio::main]
pub async fn log_service(
    service_name: &str,
    job_id: &str,
    include_input: bool,
    include_response: bool,
    include_logs: bool,
    include_timer: bool,
) -> RResult<Value, AnyErr2> {
    let mut endpoint_builder = Endpoint::builder()
        .base_url(&get_server_url().await)
        .endpoint(&format!("/logs/{}/{}", service_name, job_id))
        .method(Method::GET);

    let mut query = HashMap::new();
    query.insert("input".to_string(), include_input.to_string());
    query.insert("response".to_string(), include_response.to_string());
    query.insert("logs".to_string(), include_logs.to_string());
    query.insert("timer".to_string(), include_timer.to_string());

    endpoint_builder = endpoint_builder.query_params(query);
    let endpoint = endpoint_builder.build().unwrap();

    let response = endpoint
        .send()
        .await
        .change_context(err2!("Failed to retrieve logs"))?;

    let log_data: &serde_json::Map<String, Value> = response
        .as_object()
        .ok_or_else(|| err2!("Response is not an object"))?;

    // Initialize the main table
    let mut main_table = Table::new();
    main_table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_width(180);

    // Input section
    // if include_input {
    if let Some(validated_input) = log_data.get("validated_input") {
        let mut input_table = Table::new();
        input_table.set_header(vec![
            Cell::new("User Input").add_attribute(comfy_table::Attribute::Bold)
        ]);

        let pretty_input = match validated_input.as_str() {
            Some(validated_str) => match serde_json::from_str::<Value>(validated_str) {
                Ok(json_value) => serde_json::to_string_pretty(&json_value)
                    .unwrap_or_else(|_| validated_str.to_string()),
                Err(_) => validated_str.to_string(),
            },
            None => validated_input.to_string(),
        };

        input_table.add_row(vec![Cell::new(pretty_input)]);
        main_table.add_row(vec![Cell::new(input_table)]);
    }
    // }

    // Response section
    // if include_response {
    if let Some(response) = log_data.get("response") {
        let mut response_table = Table::new();
        response_table.set_header(vec![
            Cell::new("Server Response").add_attribute(comfy_table::Attribute::Bold)
        ]);

        let pretty_response = match response.as_str() {
            Some(str) => match serde_json::from_str::<Value>(str) {
                Ok(json_value) => {
                    serde_json::to_string_pretty(&json_value).unwrap_or_else(|_| str.to_string())
                }
                Err(_) => str.to_string(),
            },
            None => response.to_string(),
        };

        response_table.add_row(vec![Cell::new(pretty_response)]);
        main_table.add_row(vec![Cell::new(response_table)]);
    }
    // }

    // Timer section
    // if include_timer {
    let mut timer_table = Table::new();
    timer_table.set_header(vec![
        Cell::new("Timer").add_attribute(comfy_table::Attribute::Bold)
    ]);

    if let Some(started_at) = log_data.get("started_at") {
        timer_table.add_row(vec![
            Cell::new("Started At"),
            Cell::new(started_at.to_string()).set_alignment(CellAlignment::Center),
        ]);
    }

    if let Some(ended_at) = log_data.get("ended_at") {
        timer_table.add_row(vec![
            Cell::new("Ended At"),
            Cell::new(ended_at.to_string()).set_alignment(CellAlignment::Center),
        ]);
    }

    if let (Some(Value::String(started_at_str)), Some(Value::String(ended_at_str))) =
        (log_data.get("started_at"), log_data.get("ended_at"))
    {
        let started_at = DateTime::parse_from_rfc3339(started_at_str)
            .map_err(|_| err2!("Failed to parse started_at"))?;
        let ended_at = DateTime::parse_from_rfc3339(ended_at_str)
            .map_err(|_| err2!("Failed to parse ended_at"))?;

        let duration = ended_at - started_at;
        let elapsed_time = format!("{} milliseconds", duration.num_milliseconds());

        timer_table.add_row(vec![
            Cell::new("Elapsed Time"),
            Cell::new(elapsed_time).set_alignment(CellAlignment::Center),
        ]);
    }

    main_table.add_row(vec![Cell::new(timer_table)]);

    // Logs section
    // if include_logs {
    if let Some(logs) = log_data.get("logs") {
        let mut logs_table = Table::new();
        logs_table.set_header(vec![
            Cell::new("Logs").add_attribute(comfy_table::Attribute::Bold)
        ]);

        // Convert the log string to lines, reverse them, and add each line as a separate row
        let log_entries: Vec<&str> = logs.as_str().unwrap_or("").lines().collect();
        for entry in log_entries {
            logs_table.add_row(vec![Cell::new(entry).set_alignment(CellAlignment::Left)]);
        }

        main_table.add_row(vec![
            Cell::new(logs_table).set_alignment(CellAlignment::Left)
        ]);
    }
    // }

    debug!("Main Table: {:?}", "FOO");

    // Output the main table
    println!("{main_table}");

    Ok(response)
}
