mod auto_update;
mod env_checker;
mod prelude;
mod serve;
mod xp;
use auto_update::check_for_update;
use env_checker::py_env_checker;
use clap::{Parser, Subcommand};
use serve::{
    delete_service, deploy_service, jobs_service, list_services, log_service, run_tests,
    scale_service, ScaleServiceConf,
};
use std::collections::HashMap;
use std::{path::Path, process::Command};
use tracing_subscriber::{filter::EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    cmd::{run_command, run_python_script},
    files::assert_files_exist,
    prelude::*,
};
use xp::stream_logs;

static TRAIN_REPO_URL: &str = "https://github.com/Wondera-AI/mlx.git";
static PY_INF_REPO_URL: &str = "https://github.com/Wondera-AI/mlx-pyinf.git";
static SCRIPT_PATH: &str = "main.py";
static CONFIG_PATH: &str = "pyproject.toml";
static SERVICE_SCHEMA_PATH: &str = "schema.json";
static SERVICE_TOML_PATH: &str = "mlx.toml";
static SERVICE_DOCKERFILE_PATH: &str = "Dockerfile";
static RAY_ADDRESS: &str = "auto";
// static SERVER_ADDRESS: &str = "http://3.132.162.86:30000";

#[derive(Parser)]
#[command(name = "MLX")]
#[command(about = "Machine Learning Experiments", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Deploy services")]
    Serve {
        #[command(subcommand)]
        action: ServeActions,
    },
    #[command(about = "Run a workload")]
    Run {
        #[arg(help = "The workload to run")]
        workload: String,
    },
    #[command(about = "Interact with machines")]
    Machines {
        #[arg(help = "The machine to interact with")]
        machine: String,
    },
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
    #[command(about = "Control data jobs and manage filesystem")]
    Data {
        #[command(subcommand)]
        action: DataActions,
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
    #[command(about = "Initialize the service")]
    Init,
    #[command(about = "Start a new service project cloning the PINF template")]
    New {
        #[arg(help = "Name of the service")]
        name: String,
    },
    #[command(about = "Test the Service locally with tests defined in the mlx.toml")]
    Run {
        #[arg(help = "Optionally define a test name")]
        test: Option<String>,
        #[arg(long, help = "Run test call remotely", default_value = "false")]
        remote: bool,
    },
    #[command(about = "Deploy the server to a service")]
    Deploy {
        #[arg(long, help = "Docker image proxy", default_value = "false")]
        proxy: bool,

        #[arg(long, help = "Docker image name")]
        image: Option<String>,

        #[arg(long, help = "Service name")]
        name: Option<String>,

        #[arg(long, help = "Environment variables as string of dict")]
        env: Option<String>,

        #[arg(long, help = "CPU garuanteed resources")]
        cpu_requests: Option<f32>,

        #[arg(long, help = "mem garuanteed resources")]
        mem_requests: Option<u32>,

        #[arg(long, help = "GPU garuanteed resources")]
        gpu_requests: Option<u32>,

        #[arg(long, help = "Optional internal port override for proxy service")]
        internal_port: Option<i32>,

        #[arg(
            long,
            value_parser = value_parser!(String),
            value_delimiter = ',',
            value_parser = parse_key_val::<String, String>,
            help = "Node selectors for the service"
        )]
        node_selectors: Option<HashMap<String, String>>,
    },
    // (DeployServiceConf),
    #[command(about = "List the available services")]
    Ls {
        #[arg(help = "Name of the service")]
        name: Option<String>,
        // #[arg(long, help = "Show only the service pointers", default_value = "false")]
        // pointers: bool,
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

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stdout))
        .with(EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .init();

    let cli = Cli::parse();

    debug!("Check autoupdate");
    let update_enabled = std::env::var("UPDATE")
        .map(|value| value.to_lowercase() != "false")
        .unwrap_or(true);

    if update_enabled {
        check_for_update().await;
    }

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

                run_python_script("main.py", Some(&["--gen-bindings", "1"]));
            }
            TrainActions::Run => {
                info!("Running the training experiment locally");

                assert_files_exist(vec!["main.py", "pyproject.toml"]);

                py_env_checker(false);

                run_python_script("main.py", Some(&["--gen-bindings", "0"]));
            }
            TrainActions::Launch {
                ray_address,
                prepare_batches,
            } => {
                info!("Launching training experiment on remote Ray cluster");

                assert_files_exist(vec!["main.py", "pyproject.toml"]);

                py_env_checker(false);

                run_python_script(
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
            ServeActions::Init => {
                info!("Initializing the service");

                // Check if Python 3.11 is installed, if not install it
                py_env_checker(false);

                // Install project dependencies using pdm
                info!("Installing project dependencies...");
                let _ = run_command("pdm", &["install"]);

                info!("Setup complete");
            }
            ServeActions::New { name } => {
                info!("Creating new service: {}", name);

                let target_path = Path::new(&name);

                info!(
                    "Cloning the training repo to {}",
                    target_path.to_str().unwrap()
                );
                let _ = run_command(
                    "git",
                    &["clone", PY_INF_REPO_URL, target_path.to_str().unwrap()],
                );
                // Check if Python 3.11 is installed, if not install it
                py_env_checker(false);

                // Change to the newly cloned repo directory
                std::env::set_current_dir(target_path).expect("Failed to change directory");

                // Install project dependencies using pdm
                info!("Installing project dependencies...");
                let _ = run_command("pdm", &["install"]);

                info!("Setup complete for {}", name);
            }
            ServeActions::Run { test, remote } => {
                if !remote {
                    info!("Running Service locally");
                } else {
                    info!("Calling Service endpoint");
                }
                // Implement the logic to run the server locally
                assert_files_exist(vec![
                    SCRIPT_PATH,
                    CONFIG_PATH,
                    SERVICE_SCHEMA_PATH,
                    SERVICE_TOML_PATH,
                ]);

                if !remote {
                    py_env_checker(true);
                    run_python_script("main.py", Some(&["--build", "1"]));
                    assert_files_exist(vec![SERVICE_SCHEMA_PATH]);
                }

                let res = run_tests(test.clone(), *remote).await;
                res.unwrap();
            }
            ServeActions::Deploy {
                proxy,
                image,
                name,
                env,
                gpu_requests,
                cpu_requests,
                mem_requests,
                internal_port,
                node_selectors,
            } => {
                if *proxy {
                    info!("Deploying the Service to MLX as Docker proxy...");
                } else {
                    info!("Deploying the Service to a MLX...");
                    assert_files_exist(vec![
                        SCRIPT_PATH,
                        CONFIG_PATH,
                        SERVICE_TOML_PATH,
                        SERVICE_DOCKERFILE_PATH,
                    ]);

                    py_env_checker(false);

                    run_python_script("main.py", Some(&["--build", "1"]));

                    assert_files_exist(vec![SERVICE_SCHEMA_PATH]);
                }

                let _ = deploy_service(
                    *proxy,
                    name.clone(),
                    image.clone(),
                    env.clone(),
                    gpu_requests.clone(),
                    cpu_requests.clone(),
                    mem_requests.clone(),
                    node_selectors.clone(),
                    internal_port.clone(),
                )
                .await;
            }
            ServeActions::Ls { name } => {
                info!("Listing available services");

                let _ = list_services(name.as_deref()).await;
            }
            ServeActions::Rm { name, version, all } => {
                if let Some(version) = version {
                    info!("Removing service {} version {}", name, version);
                    let _ = delete_service(name, Some(*version)).await;
                } else {
                    if !all {
                        error!("Please specify a version to remove or use the --all flag to remove all versions of the service");
                    } else {
                        info!("Removing all versions of service {}", name);
                        let _ = delete_service(name, None).await;
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

                let resp = log_service(name, job_id, *input, *response, *logs, *timer);
                resp.await.unwrap();
            }
            ServeActions::Jobs { name } => {
                info!("Viewing jobs for service {}", name);

                let _ = jobs_service(name);
            }
        },
        Commands::Run { workload } => {
            info!("Running workload: {}", workload);
            // Implement the logic to run the workload
        }
        Commands::Machines { machine } => {
            info!("Interacting with machine: {}", machine);
            // Implement the logic to interact with the machine
        }
    }
}

fn parse_key_val<T, U>(
    s: &str,
) -> Result<(T, U), Box<dyn std::error::Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: std::error::Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}
