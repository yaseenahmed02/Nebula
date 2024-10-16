# Nebula

This repository contains a distributed application built using Rust and gRPC (Tonic). Follow the instructions below to set up and run the server and client on your local machine.

## Prerequisites

Before running the application, ensure you have the following installed:

- **Rust**: The programming language used for this project.
- **Protocol Buffers**: For serializing structured data.

### Installing Rust

To install Rust, follow these steps:

1. Open your terminal.
2. Run the following command to download and install Rust:

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. After installation, ensure the `cargo` command is in your PATH. You can add it by running:

   ```bash
   source $HOME/.cargo/env
   ```

4. Verify the installation by checking the version:

   ```bash
   rustc --version
   ```

### Installing Protocol Buffers

To install Protocol Buffers on Ubuntu, use the following command:

```bash
sudo apt-get install protobuf-compiler
```

Verify the installation by checking the version:

```bash
protoc --version
```

## Setting Up the Project

1. Clone the repository:

   ```bash
   git clone <repository-url>
   cd <repository-directory>
   ```

2. Navigate to the server directory and build the server:

   ```bash
   cd nebula/server
   cargo build
   ```

3. Navigate to the client directory and build the client:

   ```bash
   cd ../client
   cargo build
   ```

## Running the Application

### Start the Server

1. Open a terminal and navigate to the server directory:

   ```bash
   cd nebula/server
   ```

2. Run the server:

   ```bash
   cargo run
   ```

### Start the Client

1. Open another terminal and navigate to the client directory:

   ```bash
   cd nebula/client
   ```

2. Run the client:

   ```bash
   cargo run
   ```

### Expected Output

When you run the client, you should see a response similar to:

```
Response: "HELLO, TONIC!"
```

## Troubleshooting

- If you encounter issues with ports, ensure no other services are using the required ports. You can check active services with:

  ```bash
  sudo lsof -i :50051
  ```

- If Rust is not recognized as a command, ensure the installation directory is added to your PATH.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
