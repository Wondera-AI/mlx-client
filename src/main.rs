// use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use serde::de;
// use serde::{Deserialize, Serialize};
// use serde_json::{json, Number};
// use serde_yaml::{from_str, Value};
use std::{
    // collections::HashMap,
    // env,
    // error::Error,
    path::Path,
    process::Command,
};
mod prelude;
mod serve;
mod train;
mod xp;
pub use reqwest::Method;
use serve::{delete_service, deploy_service, list_services};
use tracing::{error, info, Level};
use train::{assert_files_exist, run_python_script_with_args};
use xp::stream_logs;

static TRAIN_REPO_URL: &str = "https://github.com/Wondera-AI/mlx.git";
static SCRIPT_PATH: &str = "main.py";
static CONFIG_PATH: &str = "pyproject.toml";
static SERVICE_CONFIG_PATH: &str = "config.json";
static RAY_ADDRESS: &str = "auto";

#[derive(Parser)]
#[command(name = "MLX")]
#[command(about = "Machine Learning Experiments", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Control training experiments")]
    Train {
        #[command(subcommand)]
        action: TrainActions,
    },
    #[command(about = "Manage experiment runs and view results")]
    Xp {
        #[command(subcommand)]
        action: XpActions,
    },
    #[command(about = "Control data jobs and manage fs-pvc")]
    Data {
        #[command(subcommand)]
        action: DataActions,
    },
    #[command(about = "Control deployed services")]
    Serve {
        #[command(subcommand)]
        action: ServeActions,
    },
}

#[derive(Subcommand)]
enum TrainActions {
    #[command(about = "Creates a new training experiment folder from this template")]
    New {
        #[arg(help = "The name of the new training experiment")]
        name: String,
    },
    #[command(
        about = "Automatically generate the configuration yaml from the experiment definition"
    )]
    Bind,
    #[command(about = "Locally run the training experiment to test prior to launching")]
    Run,
    #[command(about = "Run the training experiment on a remote Ray cluster")]
    Launch {
        #[arg(long, env, help = "Address definable also as an environment variable RAY_ADDRESS", default_value = RAY_ADDRESS)]
        ray_address: String,
        #[arg(
            long,
            help = "Create and save Ray datasets that map batches according to a user-defined Dataset prior to the model being trained for greater performance on each batch iteration."
        )]
        prepare_batches: bool,
    },
}

#[derive(Subcommand)]
enum XpActions {
    #[command(about = "Lists the experiments run remotely")]
    Ls,
    #[command(about = "Streamed stdout of remote experiment jobs")]
    Logs {
        #[arg(help = "Name of the experiment")]
        name: String,
        #[arg(help = "Run identifier of the experiment")]
        run: String,
    },
    #[command(about = "Live tensorboards of a particular experiment")]
    Board {
        #[arg(help = "Name of the experiment")]
        name: String,
        #[arg(help = "Run identifier of the experiment")]
        run: String,
    },
    #[command(about = "Ray cluster monitor to view jobs, logs, and cluster-specific metrics")]
    Ray,
}

#[derive(Subcommand)]
enum DataActions {
    #[command(about = "Displays filesystem structure of shared NFS")]
    Show,
    #[command(about = "Creates a new arbitrary data job folder from another template")]
    New,
    #[command(about = "Run data job locally")]
    Run,
    #[command(about = "Run data job on a remote Ray cluster")]
    Launch {
        #[arg(
            long,
            env,
            help = "Address definable also as an environment variable RAY_ADDRESS"
        )]
        ray_address: Option<String>,
    },
    #[command(about = "Remove a folder from the shared NFS")]
    Rm,
}

#[derive(Args, Clone)]
struct ServeConfig {
    #[arg(long, help = "Name of the service")]
    service_name: String,
    #[arg(
        long,
        help = "CPU cores or milicores limit requested",
        required_unless_present = "gpu_limit"
    )]
    cpu_limit: Option<String>,
    // #[arg(long, help = "Use available GPU devices")]
    // use_gpu: Option<bool>,
    #[arg(
        long,
        help = "GPU cores or milicores requested",
        // required_unless_present = "use_gpu"
    )]
    gpu_limit: Option<String>,
    #[arg(long, help = "Memory limit requested")]
    memory_limit: String,
    #[arg(long, help = "Number of concurrent jobs available per Service")]
    concurrent_jobs: i8,
    // #[arg(long, help = "Maximum request rate limit")]
    // max_rate_limit: i8,
}

#[derive(Subcommand)]
enum ServeActions {
    #[command(about = "Run the server locally")]
    Run,
    #[command(about = "Deploy the server to a service")]
    Deploy(ServeConfig),
    // Deploy {
    //     #[arg(help = "Name of the service")]
    //     conf: ServeConfig,
    // },
    #[command(about = "List the available services")]
    Ls {
        #[arg(long, help = "Name of the service")]
        name: Option<String>,
    },
    #[command(about = "Remove a service")]
    Rm {
        #[arg(help = "Name of the service")]
        name: String,
        #[arg(
            help = "Optional version of the service - will delete all under name if not specified"
        )]
        version: Option<u32>,
        #[arg(
            long,
            help = "Force delete all versions of the service",
            default_value = "false"
        )]
        all: bool,
    },
    #[command(about = "View the logs of a service")]
    Logs {
        #[arg(help = "Name of the service")]
        name: String,
    },
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Train { action } => match action {
            TrainActions::New { name } => {
                info!("Creating new training experiment: {}", name);

                let target_path = Path::new(&name);

                // Create the directory
                if let Err(e) = std::fs::create_dir(target_path) {
                    error!("Failed to create directory {}: {}", name, e);
                    return;
                }

                // Clone the repository
                let status = Command::new("git")
                    .arg("clone")
                    .arg(TRAIN_REPO_URL)
                    .arg(target_path)
                    .status()
                    .expect("Failed to execute git command");

                if !status.success() {
                    eprintln!("Failed to clone repository");
                    return;
                }

                // Check if Python 3.11 is installed, if not install it
                py_env_checker(false);

                // Change to the newly cloned repo directory
                std::env::set_current_dir(target_path).expect("Failed to change directory");

                // Install project dependencies using pdm
                info!("Installing project dependencies...");
                Command::new("pdm")
                    .arg("install")
                    .status()
                    .expect("Failed to install project dependencies");

                info!("Setup complete for {}", name);
            }
            TrainActions::Bind => {
                info!("Generating configuration YAML from experiment definition");

                assert_files_exist(vec![SCRIPT_PATH, CONFIG_PATH]);

                py_env_checker(false);

                run_python_script_with_args("main.py", Some(&["--gen-bindings", "1"]));
            }
            TrainActions::Run => {
                info!("Running the training experiment locally");

                assert_files_exist(vec!["main.py", "pyproject.toml"]);

                py_env_checker(false);

                run_python_script_with_args("main.py", Some(&["--gen-bindings", "0"]));
            }
            TrainActions::Launch {
                ray_address,
                prepare_batches,
            } => {
                info!("Launching training experiment on remote Ray cluster");

                assert_files_exist(vec!["main.py", "pyproject.toml"]);

                py_env_checker(false);

                run_python_script_with_args(
                    "main.py",
                    Some(&[
                        "--gen-bindings",
                        "0",
                        "--ray-address",
                        ray_address,
                        "--prepare-batches",
                        &prepare_batches.to_string(),
                    ]),
                );
            }
        },
        Commands::Xp { action } => match action {
            XpActions::Ls => {
                println!("Listing remote experiments");
                // Implement the logic to list experiments run remotely
            }
            XpActions::Logs { name, run } => {
                info!("Streaming logs for experiment {} run {}", name, run);

                let result = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(stream_logs());

                if let Err(e) = result {
                    println!("Error occurred: {:?}", e);
                }
            }
            XpActions::Board { name, run } => {
                println!(
                    "Displaying live TensorBoard for experiment {} run {}",
                    name, run
                );
                // Implement the logic to show live TensorBoard
            }
            XpActions::Ray => {
                println!("Displaying Ray cluster monitor");
                // Implement the logic to view Ray jobs, logs, and metrics
            }
        },
        Commands::Data { action } => match action {
            DataActions::Show => {
                println!("Displaying filesystem structure of shared NFS");
                // Implement the logic to show filesystem structure
            }
            DataActions::New => {
                println!("Creating new data job folder from template");
                // Implement the logic to create a new data job folder
            }
            DataActions::Run => {
                println!("Running data job locally");
                // Implement the logic to run data job locally
            }
            DataActions::Launch { ray_address } => {
                println!("Launching data job on remote Ray cluster");
                if let Some(address) = ray_address {
                    println!("Ray address: {}", address);
                }
                // Implement the logic to launch the data job on a remote Ray cluster
            }
            DataActions::Rm => {
                println!("Removing folder from shared NFS");
                // Implement the logic to remove a folder from the shared NFS
            }
        },
        Commands::Serve { action } => match action {
            ServeActions::Run => {
                println!("Running the server locally");
                // Implement the logic to run the server locally
                assert_files_exist(vec![SCRIPT_PATH, CONFIG_PATH, SERVICE_CONFIG_PATH]);

                py_env_checker(false);
            }
            ServeActions::Deploy(conf) => {
                info!("Deploying the Service to a MLX cluster...");

                assert_files_exist(vec![SCRIPT_PATH, CONFIG_PATH, SERVICE_CONFIG_PATH]);

                py_env_checker(true);

                run_python_script_with_args("main.py", Some(&["--build"]));

                let _ = deploy_service(conf);
            }
            ServeActions::Ls { name } => {
                info!("Listing available services");

                list_services(name.as_deref());
            }
            ServeActions::Rm { name, version, all } => {
                if let Some(version) = version {
                    info!("Removing service {} version {}", name, version);
                    delete_service(name, Some(*version));
                } else {
                    if !all {
                        error!("Please specify a version to remove or use the --all flag to remove all versions of the service");
                    } else {
                        info!("Removing all versions of service {}", name);
                        delete_service(name, None);
                    }
                }
            }
            ServeActions::Logs { name } => {
                println!("Viewing logs for service {}", name);
                // Implement the logic to view the logs of a service
            }
        },
    }
}

fn py_env_checker(install: bool) -> bool {
    // Check if Python 3.11 is installed, if not install it
    let python_installed = Command::new("python3.11").arg("--version").output().is_ok();

    if !python_installed {
        info!("Python 3.11 is not installed. Installing Python 3.11...");
        if cfg!(target_os = "linux") {
            Command::new("sudo")
                .args(["apt-get", "update"])
                .status()
                .expect("Failed to update package list");

            Command::new("sudo")
                .args(["apt-get", "install", "-y", "python3.11"])
                .status()
                .expect("Failed to install Python 3.11");

            // return true;
        } else if cfg!(target_os = "macos") {
            Command::new("brew")
                .args(["install", "python@3.11"])
                .status()
                .expect("Failed to install Python 3.11");

            // return true;
        } else {
            error!("Automatic Python 3.11 installation is not supported on this OS.");

            return false;
        }
    }

    // Install pdm
    let pdm_installed = Command::new("pdm").arg("info").output().is_ok();

    if !pdm_installed {
        info!("Installing PDM...");
        Command::new("python3.11")
            .args(["-m", "pip", "install", "--user", "pdm"])
            .status()
            .expect("Failed to install PDM");
    }

    info!("Python3.11 & PDM all ok");

    if install {
        info!("Installing PDM dependencies");

        Command::new("pdm")
            .arg("install")
            .status()
            .expect("Failed to install PDM dependencies");
    }

    return true;
}
