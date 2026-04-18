# WSL Linux Build Setup Guide

This guide describes how to set up a Linux build environment for YATS directly in WSL (Windows Subsystem for Linux). This is an alternative to using Docker.

## Prerequisites

- WSL2 installed (with Ubuntu 22.04 recommended)

## GLIBC compatibility note

If your target Linux distribution is older (e.g., Ubuntu 22.04 or older), you must build on a compatible environment.
The default `Ubuntu` in WSL is typically version 24.04, which links against a newer GLIBC (2.39). Binaries built there will not run on older systems (like Ubuntu 20.04/22.04).

**Recommendation:** Install and use `Ubuntu-22.04` in WSL.

```bash
# Install Ubuntu 22.04
wsl --install -d Ubuntu-22.04

# Run build in this specific distro
wsl -d Ubuntu-22.04
```

## Setup Steps

Open your WSL terminal and run the following commands:

### 1. Update and Install System Dependencies

```bash
sudo apt-get update
sudo apt-get install -y \
    curl \
    wget \
    build-essential \
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libxdo-dev \
    libsoup-3.0-dev \
    libssl-dev \
    pkg-config \
    libevdev-dev \
    xdg-utils \
    git
```

### 2. Install Node.js (v20)

```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
sudo apt-get install -y nodejs
```

### 3. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env
```

## Building the Project

1. Navigate to the project directory in WSL:
   ```bash
   cd /path/to/yats
   ```
2. Install dependencies:
   ```bash
   npm install
   ```
3. Run the Tauri build:
   ```bash
   npm run tauri build
   ```

The binaries will be generated in `src-tauri/target/release/bundle/`.
