use clap::{Args, Parser, Subcommand};
use std::{path::Path, process::Command};
mod prelude;
mod serve;
mod train;
mod xp;
pub use reqwest::Method;
use serve::{
    delete_service, deploy_service, jobs_service, list_services, log_service, scale_service,
    DeployServiceConf, ScaleServiceConf,
};
use tracing::{error, info, Level};
use train::{assert_files_exist, run_python_script_with_args};
use utils::cmd::run_command;
use xp::stream_logs;

static TRAIN_REPO_URL: &str = "https://github.com/Wondera-AI/mlx.git";
static PY_INF_REPO_URL: &str = "https://github.com/Wondera-AI/mlx-pyinf.git";
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

#[derive(Subcommand)]
enum ServeActions {
    #[command(about = "Start a new service project cloning the PINF template")]
    New {
        #[arg(help = "Name of the service")]
        name: String,
    },
    #[command(about = "Run the server locally")]
    Run {
        #[arg(help = "Run the server in debug mode")]
        test: Option<u32>,
    },
    #[command(about = "Deploy the server to a service")]
    Deploy(DeployServiceConf),
    #[command(about = "List the available services")]
    Ls {
        #[arg(help = "Name of the service")]
        name: Option<String>,
        #[arg(long, help = "Show only the service pointers", default_value = "false")]
        pointers: bool,
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
    #[command(about = "Scale the service")]
    Scale(ScaleServiceConf),
    #[command(about = "View the logs of a service")]
    Logs {
        #[arg(help = "Name of the service")]
        name: String,
        #[arg(help = "Job ID of the service")]
        job_id: String,
        #[arg(
            long,
            help = "Include validated input in the logs",
            default_value_t = false
        )]
        input: bool,
        #[arg(long, help = "Include response in the logs", default_value_t = false)]
        response: bool,
        #[arg(
            long,
            help = "Include pod job logs in the output",
            default_value_t = false
        )]
        logs: bool,
        #[arg(long, help = "Include timer information", default_value_t = false)]
        timer: bool,
    },
    #[command(about = "View the jobs of a service")]
    Jobs {
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
            ServeActions::New { name } => {
                info!("Creating new service: {}", name);

                let target_path = Path::new(&name);

                info!(
                    "Cloning the training repo to {}",
                    target_path.to_str().unwrap()
                );
                run_command(
                    "git",
                    &["clone", PY_INF_REPO_URL, target_path.to_str().unwrap()],
                );
                // Check if Python 3.11 is installed, if not install it
                py_env_checker(false);

                // Change to the newly cloned repo directory
                std::env::set_current_dir(target_path).expect("Failed to change directory");

                // Install project dependencies using pdm
                info!("Installing project dependencies...");
                run_command("pdm", &["install"]);

                info!("Setup complete for {}", name);
            }
            ServeActions::Run { test } => {
                println!("Running the server locally");
                // Implement the logic to run the server locally
                assert_files_exist(vec![SCRIPT_PATH, CONFIG_PATH, SERVICE_CONFIG_PATH]);

                py_env_checker(false);
            }
            ServeActions::Deploy(conf) => {
                info!("Deploying the Service to a MLX cluster...");

                assert_files_exist(vec![SCRIPT_PATH, CONFIG_PATH]);

                py_env_checker(true);

                run_python_script_with_args("main.py", Some(&["--build", "1"]));

                assert_files_exist(vec![SERVICE_CONFIG_PATH]);

                let _ = deploy_service(conf);
            }
            ServeActions::Ls { name, pointers } => {
                info!("Listing available services");

                list_services(name.as_deref(), *pointers);
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
            ServeActions::Scale(conf) => {
                info!("Scaling the service");

                let _ = scale_service(conf);
            }
            ServeActions::Logs {
                name,
                job_id,
                input,
                response,
                logs,
                timer,
            } => {
                info!("Viewing logs for service: {} with job_id: {}", name, job_id);

                log_service(name, job_id, *input, *response, *logs, *timer);
            }
            ServeActions::Jobs { name } => {
                info!("Viewing jobs for service {}", name);

                jobs_service(name);
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
