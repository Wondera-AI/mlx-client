#!/bin/bash

# Install required system dependencies
echo "Installing system dependencies..."
if [ -x "$(command -v apt)" ]; then
    sudo apt update
    sudo apt install -y build-essential git curl pkg-config libssl-dev
elif [ -x "$(command -v yum)" ]; then
    sudo yum groupinstall -y "Development Tools"
    sudo yum install -y pkg-config openssl-devel
elif [ -x "$(command -v dnf)" ]; then
    sudo dnf groupinstall -y "Development Tools"
    sudo dnf install -y pkg-config openssl-devel
elif [ -x "$(command -v pacman)" ]; then
    sudo pacman -S --noconfirm base-devel git curl pkgconf openssl
else
    echo "Unsupported package manager. Please install build-essential or equivalent tools manually."
    exit 1
fi

# Check if Rust is installed
if ! command -v rustc &> /dev/null
then
    echo "Rust not found, installing Rust and Cargo..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
else
    echo "Rust is already installed."
fi

# add to path
echo 'export PATH="$HOME/.cargo/bin:$PATH"' | tee -a ~/.zshrc ~/.bash_profile > /dev/null && export PATH="$HOME/.cargo/bin:$PATH"

# Verify installation
rustc --version
cargo --version

# Clone the public repository
REPO_URL="https://github.com/Wondera-AI/mlx-client.git"
REPO_DIR="mlx-client"
BINARY_NAME="mlx"

# Remove the existing installation if it exists
if cargo install --list | grep -q "$BINARY_NAME "; then
    echo "Removing the existing installation of $BINARY_NAME..."
    cargo uninstall "$BINARY_NAME"
fi

if [ -d "$REPO_DIR" ]; then
    echo "Directory $REPO_DIR already exists. Deleting it to clone afresh."
    rm -rf "$REPO_DIR"
fi

echo "Cloning the MLX-Client repository..."
git clone "$REPO_URL"
cd "$REPO_DIR"

# Build and install the project
echo "Building and installing MLX-Client"
cargo build --release
cargo install --path .

# Delete the repository
echo "Deleting the repository..."
cd ..
rm -rf "$REPO_DIR"

echo "MLX installed. To use type in bash/zsh 'mlx --help'"