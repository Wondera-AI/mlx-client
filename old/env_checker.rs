use std::process::Command;
use utils::{cmd::run_command, prelude::*};

pub fn py_env_checker(install: bool) -> bool {
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

    let pdm_installed = Command::new("pdm").arg("info").output().is_ok();

    if !pdm_installed {
        info!("Installing PDM...");
        if cfg!(target_os = "linux") {
            let _ = run_command("sudo apt install python3-venv", &[]);
        }
        let _ = run_command(
            "curl -sSL https://pdm-project.org/install-pdm.py | python3 -",
            &[],
        );
    }
    let path_setup = r"
    echo 'export PATH=$HOME/.local/bin:$PATH' >> ~/.bashrc;
    echo 'export PATH=$HOME/.local/bin:$PATH' >> ~/.zshrc;
    export PATH=$HOME/.local/bin:$PATH;
    ";
    let _ = run_command(path_setup, &[]);
    info!("Python3.11 & PDM all ok");

    if install {
        info!("Installing PDM dependencies");

        Command::new("pdm")
            .arg("install")
            .status()
            .unwrap_or_else(|_| panic!("IF THIS FAILS, YOUR PYTHON SETUP IS UNIQUE TO ALL OTHER WONDERA MACHINE - CALL ALE TO SUPPORT YOUR SETUP"));
    }

    return true;
}
