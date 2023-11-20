# Deploy Tool

## Overview

`Deploy` is a custom tool designed to streamline the process of deploying applications and services. Similar to Ansible but with a simpler design, it focuses on the essentials: executing commands on remote SSH servers, transferring folders, and streaming remote terminal sessions. Its operation is guided by TOML configuration files.

### Key Features

- **Command Execution:** Automate command execution on remote servers via SSH.
- **File Transfer:** Easily transfer directories to your remote server.
- **Selective Sync:** Utilize `.deployignore` to ignore specific files or directories, mimicking `.gitignore` functionality.
- **Logging:** Automatically generates deployment logs in the `.deployments` directory.

## Getting Started

### Installation

Deploy is built using Rust. To install:

```bash
cargo build --release
cp target/release/deploy ~/.local/bin/
```

### Usage

#### Basic Commands

- `deploy --help`: Display help information.
- `deploy --find .`: List available deployment configurations in the current and subdirectories.
- `deploy <file.deploy.toml>`: Start the deployment process as per the specified TOML file.
- `deploy <file.deploy.toml> --skip action1,action2`: Start deployment as per the specified TOML file, but skip the listed actions.

### Configuration File Format

Deployment configurations are written in TOML. Here's a basic structure:

```'toml
[server]
host = "ssh://146.59.159.230"
user = "ubuntu"
ssh_key = "~/.ssh/termius"

[[actions]]
type = "commands"
name = "stop"
commands = [
    "cd api.starknet.id/",
    "sudo docker-compose -f docker-compose.prod.yml down",
]

[[actions]]
type = "upload"
name = "upload_sources"
source_folder = "~/starknetid/api.starknet.id/"
target_folder = "~/api.starknet.id/"

[[actions]]
type = "upload"
name = "upload_configs"
source_folder = "~/starknetid/api.starknet.id/backups/goerli"
target_folder = "~/api.starknet.id/"

[[actions]]
type = "commands"
name = "start"
commands = [
    "cd api.starknet.id/",
    "sudo docker-compose -f docker-compose.prod.yml up --build",
]
```

### Contributing

Feedback and contributions are welcome. Please feel free to submit issues and pull requests to the repository.
