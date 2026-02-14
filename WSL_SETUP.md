# WSL Linux Build Setup Guide

This guide describes how to set up a Linux build environment for YATS directly in WSL (Windows Subsystem for Linux). This is an alternative to using Docker.

## Prerequisites

- WSL2 installed (with Ubuntu 22.04 recommended)

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
   cd /mnt/c/Users/u/OneDrive/devel/Antigravity/Projects/yats
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
