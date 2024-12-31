use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use tracing::info;
use utils::{cmd::run_command, errors::prelude::*};

pub fn build_tag_and_push_image(
    _service_id: &str,
    image_uri: &str,
    arch: &str,
) -> RResult<(), AnyErr2> {
    let platform = match arch {
        "amd64" => "linux/amd64",
        "arm64" => "linux/arm64",
        other => panic!("Unsupported architecture: {other}"),
    };

    // run_command("podman", &["system", "prune", "-a", "-f"])
    //     .change_context(err2!("Failed to prune images"))?;

    let mut args = vec![
        "docker", "build", "-t", image_uri, ".",
        // "--no-cache"
    ];

    if !platform.is_empty() {
        args.push("--platform");
        args.push(platform);
    }

    print!("Args: {:?}", args);
    run_command("sudo", &args).change_context(err2!("Failed to build image"))?;

    login().change_context(err2!("Failed to login to image registry"))?;

    info!("Pushing image to registry... (this may take a few minutes)");

    run_command(
        "docker",
        &[
            "push",
            // "--compression-format=gzip ",
            // "--compression-level=9 ",
            // "--force-compression",
            // "--tls-verify=false",
            image_uri,
        ],
    )
    .change_context(err2!("Failed to push image"))?;

    info!("Removing local image...");

    // run_command("docker", &["rmi", image_uri])
    //     .change_context(err2!("Failed to remove the image"))?;

    Ok(())
}

fn login() -> RResult<(), AnyErr2> {
    let password = "R$G5#XFY&xVMn6IJ";

    let mut cmd = Command::new("docker")
        .arg("login")
        .arg("https://h.nodestaking.com/")
        .arg("--username")
        .arg("wondera")
        .arg("--password-stdin")
        .stdin(Stdio::piped()) // Open a pipe to write to stdin
        .spawn()
        .change_context(err2!("Failed to spawn login command"))?;

    // Write the password to stdin
    if let Some(mut stdin) = cmd.stdin.take() {
        stdin
            .write_all(password.as_bytes())
            .change_context(err2!("Failed to write to stdin"))?;
    }

    // Wait for the command to finish
    let output = cmd
        .wait_with_output()
        .change_context(err2!("Failed to wait for command"))?;

    // Print output for debugging (optional)
    if !output.status.success() {
        eprintln!("Command failed with output: {:?}", output);
    } else {
        println!("Login successful!");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_success() {
        let result = login();
        assert!(result.is_ok(), "Login should succeed");
    }
}
