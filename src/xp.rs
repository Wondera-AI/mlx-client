use anyhow::Result;
use redis::AsyncCommands;
use regex::Regex;
use tracing::{error, info};

// mod utils;
use redis::Commands;
use utils::redis_manager::RedisManager;

#[derive(Debug, Clone, PartialEq)]
struct TrainingMetrics {
    training_iteration: Option<usize>,
    batch: Option<usize>,
    epoch: Option<usize>,
}

impl TrainingMetrics {
    fn new() -> Self {
        Self {
            training_iteration: None,
            batch: None,
            epoch: None,
        }
    }
}

fn _parse_training_output(line: &str, metrics: &mut TrainingMetrics) {
    let re_iteration = Regex::new(r"training_iteration\s+(\d+)").unwrap();
    let re_batch = Regex::new(r"batch\s+(\d+)").unwrap();
    let re_epoch = Regex::new(r"epoch\s+(\d+)").unwrap();

    if let Some(caps) = re_iteration.captures(line) {
        metrics.training_iteration = Some(caps[1].parse().unwrap_or(0));
    }
    if let Some(caps) = re_batch.captures(line) {
        metrics.batch = Some(caps[1].parse().unwrap_or(0));
    }
    if let Some(caps) = re_epoch.captures(line) {
        metrics.epoch = Some(caps[1].parse().unwrap_or(0));
    }
}

fn customize_data_loader(metrics: &TrainingMetrics) {
    error!(
        "Customizing data loader for epoch {:?}, batch {:?}, training iteration {:?}",
        metrics.epoch, metrics.batch, metrics.training_iteration
    );

    // Implement your data loader customization logic here
}

pub async fn stream_logs() -> Result<()> {
    let connection_string = "redis://:MkiTVpOWFVLGLgJ7ptZ29dY80zER4cvR@redis-17902.c322.us-east-1-2.ec2.redns.redis-cloud.com:17902";

    let mut redis = RedisManager::new(connection_string)?;

    let queue_name = "my_experiment_stdout";

    info!("Reading from Redis queue: {}", queue_name);

    let mut metrics = TrainingMetrics::new();
    // let mut buffer: Vec<TrainingMetrics> = Vec::new();

    // loop {
    //     match redis
    //         .client
    //         .blpop::<&str, (String, String)>(queue_name, 0.0)?
    //     {
    //         Ok(log_entry) => {
    //             let line = log_entry.1.clone();

    //             // info!("Log: {}", log_entry.1);

    //             if log_entry.1.contains("is_done") {
    //                 info!("Experiment completed, exiting...");
    //                 break;
    //             }

    //             if line.contains("training_iteration")
    //                 || line.contains("batch")
    //                 || line.contains("epoch")
    //             {
    //                 let old_metrics = metrics.clone();

    //                 _parse_training_output(&line, &mut metrics);

    //                 if metrics != old_metrics {
    //                     // buffer.push(metrics.clone());
    //                     customize_data_loader(&metrics);
    //                 }
    //             }
    //         }
    //         Err(e) => {
    //             error!("Error fetching logs from Redis: {:?}", e);
    //             break;
    //             // tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    //         }
    //     }
    // }

    Ok(())
}
