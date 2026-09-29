# DotPilot

**A Linux dotfiles and configuration manager built with Rust and Tauri.**

DotPilot is an open-source desktop application designed to simplify the management, backup, versioning, and restoration of Linux configuration files across machines.

It aims to provide a centralized interface for organizing dotfiles, tracking configuration changes, managing symlinks, and maintaining consistent development environments without manually juggling configuration files and Git repositories.

> **Status:** Early Development

---

## Overview

Linux users often customize their systems through configuration files, commonly known as *dotfiles*. These files control everything from shell behavior and terminal appearance to window manager settings, editor preferences, and application configurations.

Managing these files across multiple machines can become complicated. Configurations may be scattered across different directories, changes can be difficult to track, and restoring a previous setup often requires manual intervention.

DotPilot aims to address these challenges by providing a unified desktop interface for managing configuration files and their associated version history.

The project is designed around a few core principles:

* **Centralized management:** Keep track of configurations from a single interface.
* **Version control:** Track changes and preserve snapshots of configuration states.
* **Safe operations:** Make configuration changes transparent and provide control over backup and restoration.
* **Cross-machine consistency:** Organize configurations for different systems and simplify migration.
* **Open source:** Keep the project transparent, extensible, and community-driven.

---

## Key Features

The following features are planned and may evolve as development progresses.

### Configuration Management

Manage configuration files and directories from a centralized interface.

* Add individual configuration files or entire directories.
* Organize configurations by application or category.
* Track the source and destination of managed configurations.
* View and manage the configurations associated with a system.

### Version Control & Snapshots

Use Git-based versioning to track configuration changes over time.

* Create snapshots of configuration states.
* Maintain a history of changes.
* Compare current configurations against previous snapshots.
* Restore configurations from earlier versions.

### Backup & Restore

Provide a structured workflow for preserving and recovering configuration files.

* Back up selected files and directories.
* Restore configurations from saved snapshots.
* Review changes before restoring a previous state.
* Reduce the risk of accidentally overwriting existing configurations.

### Configuration Diff Viewer

Inspect configuration changes before applying them.

* Compare current and previous versions of files.
* Identify added, removed, and modified content.
* Review changes before committing or restoring configurations.

### Symlink Management

Simplify the process of linking configuration files to their intended locations.

* Manage configuration symlinks.
* Track source and target paths.
* Inspect existing links and identify potential conflicts.
* Support safer configuration deployment and restoration workflows.

### Machine Profiles

Organize configurations for different systems and environments.

* Maintain separate configuration sets for different machines.
* Associate configurations with specific profiles.
* Keep machine-specific settings separate from shared configurations.

### Configuration Migration

Make it easier to transfer a familiar Linux setup to another machine.

* Organize configurations into portable sets.
* Support profile-based configuration deployment.
* Simplify restoring a system's preferred configuration layout on a new machine.

---

## Technology Stack

DotPilot is planned around a native Rust backend and a modern desktop interface.

| Technology   | Purpose                                            |
| ------------ | -------------------------------------------------- |
| Rust         | Core application logic and system-level operations |
| Tauri        | Desktop application framework                      |
| React        | User interface                                     |
| TypeScript   | Type-safe frontend development                     |
| Tailwind CSS | Interface styling                                  |
| Git          | Configuration versioning and snapshots             |

The final architecture and dependencies may change during development.

---

## Architecture

DotPilot is intended to separate the desktop interface from the underlying configuration-management logic.

### Frontend

The frontend will provide the desktop interface for browsing configurations, viewing changes, managing profiles, and initiating backup or restore operations.

### Backend

The Rust backend will handle filesystem operations, configuration tracking, symlink management, and interactions with Git.

### Versioning

Git is planned as the foundation for tracking configuration history and maintaining snapshots of managed files.

The exact repository structure, storage model, and synchronization strategy are still under development.

---

## Roadmap

The roadmap outlines the intended development direction. Items are planned and are not yet guaranteed to be implemented.

### Phase 1: Foundation

* [ ] Establish the application architecture.
* [ ] Set up the Rust and Tauri development environment.
* [ ] Build the initial desktop interface.
* [ ] Define the configuration data model.

### Phase 2: Configuration Management

* [ ] Add configuration files and directories.
* [ ] Display and organize managed configurations.
* [ ] Track configuration paths and metadata.
* [ ] Implement basic filesystem operations.

### Phase 3: Version Control

* [ ] Integrate Git-based configuration tracking.
* [ ] Implement snapshots and version history.
* [ ] Build a configuration diff viewer.
* [ ] Support restoring previous snapshots.

### Phase 4: System Integration

* [ ] Implement symlink management.
* [ ] Add backup and restore workflows.
* [ ] Introduce safeguards for conflicting files and paths.
* [ ] Improve error handling and operation feedback.

### Phase 5: Profiles & Migration

* [ ] Introduce machine profiles.
* [ ] Support profile-based configuration organization.
* [ ] Develop configuration migration workflows.
* [ ] Explore cross-machine synchronization options.

---

## Getting Started

DotPilot is currently in early development. Installation instructions will be added once a functional development build is available.

### Prerequisites

The planned development environment includes:

* Linux
* Rust toolchain
* Node.js and a package manager
* Tauri development dependencies

Exact requirements will be documented as the project stabilizes.

### Development Setup

Clone the repository once the initial application scaffold is available:

```bash
git clone https://github.com/NiranjanD0/DotPilot.git
cd DotPilot
```

Build and development commands will be documented after the project structure and package scripts are established.

---

## Project Structure

The directory structure is not finalized. The project is intended to separate the frontend, backend, and configuration-management logic into maintainable components.

---

## Contributing

Contributions, ideas, bug reports, and feedback are welcome as DotPilot develops.

If you are interested in contributing:

1. Check the roadmap to understand the current development priorities.
2. Open an issue to discuss significant changes or proposed features.
3. Keep contributions focused and consistent with the project's architecture.
4. Submit a pull request with a clear explanation of the changes.

Contribution guidelines and development documentation will be expanded as the project matures.

---

## Security & Data Safety

DotPilot is intended to operate on user-managed configuration files, which may contain sensitive information.

The project aims to prioritize safe filesystem operations and transparent configuration changes.

Planned safeguards include:

* Reviewing changes before restoring configurations.
* Identifying conflicting files and symlinks.
* Providing clear feedback for filesystem operations.
* Avoiding unintended overwrites during configuration management.

Users should avoid committing secrets, credentials, API keys, or other sensitive information to version-controlled configuration repositories.

---

## License

DotPilot is licensed under the [MIT License](LICENSE).

See the `LICENSE` file for the full license text.

---

## Acknowledgments

DotPilot is built with the open-source ecosystem in mind and aims to make Linux configuration management more accessible and organized.

Contributions from the community will help shape the project's direction.

---

**DotPilot** · Open-source Linux configuration management.
