
# Distributed Project

This project consists of a simple client-server application built using Rust and the Tonic framework for gRPC communication. This guide will help you set up and run the server and client on your machine.

## Prerequisites

Before you begin, ensure you have the following installed:

### 1. Install Rust and Cargo

To install Rust and Cargo, you can use the following command in your terminal:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

This command will download and install Rust and Cargo. After installation, you need to configure your current shell session by running:

```bash
source $HOME/.cargo/env
```

### 2. Install Buf

Buf is a tool for managing Protocol Buffers (protobuf) files. To install Buf, you can run the following command:

```bash
# For Linux and macOS
brew install bufbuild/buf/buf

# Or download the binary directly:
curl -sSL https://github.com/bufbuild/buf/releases/latest/download/buf-Linux-x86_64 -o /usr/local/bin/buf
chmod +x /usr/local/bin/buf

# For Windows, you can use Scoop
scoop install buf
```

Make sure the installation was successful by checking the version:

```bash
buf --version
```

## Cloning the Repository

To get started, clone this repository to your local machine:

```bash
git clone <repository-url>
cd <repository-folder>
```

Replace `<repository-url>` with the actual URL of the repository and `<repository-folder>` with the folder name created after cloning.

## Project Structure

The project has two main components:

- **Server**: This handles incoming requests and processes them.
- **Client**: This sends requests to the server and receives responses.

### Folder Structure

```
/distributed
  ├── /client   # Client application code
  └── /server   # Server application code
```

## Setting Up the Server

1. **Navigate to the Server Directory:**

   ```bash
   cd distributed/server
   ```

2. **Build the Server:**

   Use Cargo to build the server application:

   ```bash
   cargo build
   ```

3. **Run the Server:**

   Start the server:

   ```bash
   cargo run
   ```

   The server should output something like:

   ```
   Server listening on [::1]:50051
   ```

   If you see an error indicating the address is already in use, consider changing the port in `main.rs` to another number (e.g., `50052`) and rebuilding.

## Setting Up the Client

1. **Open a New Terminal:**

   Leave the server running in its terminal and open a new terminal window for the client.

2. **Navigate to the Client Directory:**

   ```bash
   cd distributed/client
   ```

3. **Build the Client:**

   Use Cargo to build the client application:

   ```bash
   cargo build

4. **Run the Client:**

   Start the client:

   ```bash
   cargo run
   ```

   The client should output something like:

   ```
   Response: "HELLO, TONIC!"
   ```

## Troubleshooting

- **If You Encounter Errors:**
  - Ensure Rust and Cargo are correctly installed.
  - Check if the server is running before starting the client.
  - Make sure the ports are not blocked by firewalls or used by other applications.

