use crate::serve::get_server_url;
use chrono::{DateTime, Utc};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, CellAlignment, ContentArrangement, Table};
use std::collections::HashMap;
use utils::endpoints::{Endpoint, Method};
use utils::prelude::*;

#[tokio::main]
pub async fn jobs_service(service_name: &str) -> RResult<(), AnyErr2> {
    // Build the endpoint for fetching jobs
    let endpoint = Endpoint::builder()
        .base_url(&get_server_url().await)
        .endpoint(&format!("/jobs/{}", service_name))
        .method(Method::GET)
        .build()
        .unwrap();

    // Send the request to the server
    let response = endpoint
        .send()
        .await
        .change_context(err2!("Failed to retrieve jobs"))?;

    // Parse the response as a JSON object
    error!("Response: {:?}", response);
    let logs: HashMap<String, HashMap<String, String>> =
        serde_json::from_value(response.clone())
            .change_context(err2!("Failed to parse response"))?;

    // Prepare a table to display the job logs
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_width(180)
        .set_header(vec!["Job ID", "Start Time", "Elapsed Time", "Status"]);

    // Iterate through each job log and populate the table
    for (job_id, log) in logs.iter() {
        let start_time_str = log.get("started_at").unwrap_or(&"".to_string()).clone();
        let end_time_str = log.get("ended_at").unwrap_or(&"".to_string()).clone();

        // Parse start and end times to calculate elapsed time
        let elapsed_time = if let Ok(start_time) = DateTime::parse_from_rfc3339(&start_time_str) {
            let start_time = start_time.with_timezone(&Utc);
            if !end_time_str.is_empty() {
                if let Ok(end_time) = DateTime::parse_from_rfc3339(&end_time_str) {
                    let duration = end_time.signed_duration_since(start_time);
                    format!("{} ms", duration.num_milliseconds())
                } else {
                    "-".to_string()
                }
            } else {
                "-".to_string()
            }
        } else {
            "-".to_string()
        };

        let status = if end_time_str.is_empty() {
            "started"
        } else {
            "ended"
        }
        .to_string();

        table.add_row(vec![
            Cell::new(job_id).set_alignment(CellAlignment::Center),
            Cell::new(start_time_str).set_alignment(CellAlignment::Center),
            Cell::new(elapsed_time).set_alignment(CellAlignment::Center),
            Cell::new(status).set_alignment(CellAlignment::Center),
        ]);
    }
    // Print the table
    println!("{table}");

    Ok(())
}
