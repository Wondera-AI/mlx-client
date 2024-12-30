use std::io::Write;
use utils::prelude::*;

static APP_NAME: &str = "mlx-client";
static CLIENT_REPO_URL: &str = "https://api.github.com/repos/Wondera-AI/mlx-client/commits/main";

pub async fn check_for_update() {
    info!("Checking mlx-client for updates ...");

    let latest_hash = fetch_latest_commit_hash().await.unwrap();

    let current_hash = match read_current_commit_hash() {
        Ok(hash) => hash,
        Err(_) => String::new(),
    };

    debug!("Current hash: {}", current_hash);
    debug!("Latest hash: {}", latest_hash);

    if latest_hash != current_hash {
        info!("New version of mlx-client detected :) updating...");
        // Run the install.sh script to update
        std::process::Command::new("bash")
            .arg("-c")
            .arg("curl -sSL https://raw.githubusercontent.com/Wondera-AI/mlx-client/main/install.sh | bash")
            .status()
            .expect("Failed to update");

        write_current_commit_hash(&latest_hash).expect("Failed to write the latest commit hash");

        info!("Update complete, running mlx command now...");

        // let args: Vec<String> = std::env::args().skip(1).collect();
        // let args_str: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        // let _ = run_command("mlx", &args_str);
    }
}

async fn fetch_latest_commit_hash() -> Result<String, Box<dyn std::error::Error>> {
    let url = CLIENT_REPO_URL;
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("User-Agent", "mlx-client")
        .send()
        .await?;
    let json: serde_json::Value = response.json().await?;

    Ok(json["sha"].as_str().unwrap().to_string())
}

fn get_hash_file_path() -> std::io::Result<std::path::PathBuf> {
    // Get the appropriate config directory for the current platform
    let mut config_dir = dirs_next::config_dir().ok_or(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Unable to locate config directory",
    ))?;

    config_dir.push(APP_NAME);
    // Create the directory if it doesn't exist
    std::fs::create_dir_all(&config_dir)?;

    config_dir.push(".commit_hash");
    Ok(config_dir)
}

fn read_current_commit_hash() -> std::io::Result<String> {
    let hash_file_path = get_hash_file_path()?;
    if let Ok(hash) = std::fs::read_to_string(&hash_file_path) {
        Ok(hash.trim().to_string())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No hash file found",
        ))
    }
}

fn write_current_commit_hash(hash: &str) -> std::io::Result<()> {
    let hash_file_path = get_hash_file_path()?;
    let mut file = std::fs::File::create(hash_file_path)?;
    writeln!(file, "{}", hash)?;
    Ok(())
}
