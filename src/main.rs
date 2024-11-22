
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;
use tokio::sync::Semaphore;
use serde::{Serialize, Deserialize};
use std::fs::File;
use steganography::encoder::Encoder;
use::steganography::util::*;
use std::fs;
use tokio::time::timeout;
use tokio::time::{self, Duration, Instant};
use std::env;
use std::error::Error;
use sysinfo::System;
use tokio::signal;
use tokio::time::error::Elapsed;


#[derive(Clone, Copy, Debug)]
struct ServerState {
    down: bool,
    busy: bool
}

// impl ServerState {
//     fn new() -> Self {
//         ServerState {
//             down: false,  // Server starts as up
//             busy: false   // Server starts as not busy
//         }
//     }
// }

const MAX_THREADS: usize = 20;
lazy_static::lazy_static! {
    static ref ACTIVE_CLIENTS: Mutex<HashMap<String, bool>> = Mutex::new(HashMap::new());
    static ref THREAD_SEMAPHORE: Arc<Semaphore> = Arc::new(Semaphore::new(MAX_THREADS));
    static ref SERVER_STATE: Arc<Mutex<ServerState>> = Arc::new(Mutex::new(ServerState { down: true, busy: false })); 
}

#[derive(Serialize, Deserialize, Debug)]
enum MessageType {
    Segment(Vec<u8>),  // Represents image chunk or data segment
    Ack(i32),          // Represents an acknowledgment with a specific ID
    Request(String),   // Represents a request message
}

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    msgtype: MessageType,
}

#[derive(Clone, Debug)]
struct ImageID {
    ip_address: String,
    port_number: String,
    unique_identifier: String,
}

// Encryption function (stub)
pub fn encrypt_image(image_bytes: Vec<u8>, hidden_image_path: &str) -> Vec<u8> {
    let payload = image_bytes.clone();
    let destination_image = file_as_dynamic_image(hidden_image_path.to_string());
    let enc = Encoder::new(&payload, destination_image);
    let result = enc.encode_alpha();
    save_image_buffer(result, "hidden_message.png".to_string());
    let result_bytes = file_to_bytes(File::open("hidden_message.png").unwrap());
    result_bytes
}

// Function to serve each client
async fn serve_client(image_id: ImageID) -> Result<(), std::io::Error>  {
    //let ip = &image_id.ip_address;
    //let port = &image_id.port_number;
    //let socket_addr = format!("{}:{}", ip, port);
    // bind on your ip and leave port 0, os will pick any port  when the client recievess the ack he can see and use this for future transmission
    let ip_address = "127.0.0.1";
    let socket_addr = format!("{}:0", ip_address);
    let my_socket = UdpSocket::bind(&socket_addr)
        .await
        .expect("Failed to bind socket");
    //println!("Socket bound to: {}", socket.local_addr().unwrap());

    //Client should use this port ahead to send the image
    println!("Serving client on {}", socket_addr);
    let response_message = Message {
        msgtype: MessageType::Ack(-1), 
    };

    let serialized_message = bincode::serialize(&response_message)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let local_addr = my_socket.local_addr().expect("Failed to get local address");
    let client_port = local_addr.port();
    let client_ip = &image_id.ip_address;
    let client_socket_addr = format!("{}:{}", client_port, client_ip);


    match my_socket.send_to(&serialized_message, &client_socket_addr).await {
    Ok(bytes_sent) => println!("Sent {} bytes to client at {}", bytes_sent, socket_addr),
    Err(e) => println!("Failed to send message to client: {:?}", e),
    }


    let mut buf = [0; 1024];
    let mut received_image_data = Vec::new();
    let mut expected_seq_num = 0;

    loop {
        // why you break when you are not recieving
        let (len, addr) = match my_socket.recv_from(&mut buf).await {
            Ok(res) => res,
            Err(_) => {
                println!("Failed to receive data, stopping client handling.");
                continue;
            }
        };

        let data = &buf[..len];
        if data == b"END" {
            println!("End of transmission received from {}", addr);
            break;
        }

        if len < 4 {
            println!("Received invalid packet from {}", addr);
            continue;
        }

        let seq_num = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let payload = &data[4..];

        if seq_num == expected_seq_num {
            received_image_data.extend_from_slice(payload);
            expected_seq_num += 1;
            my_socket.send_to(&seq_num.to_be_bytes(), addr).await.expect("Failed to send ACK");
        } else {
            println!(
                "Out of order packet from {}. Expected: {}, got: {}.",
                addr, expected_seq_num, seq_num
            );
        }
    }

    println!("Received complete image data. Encrypting...");
    let encrypted_image_data = encrypt_image(received_image_data, "server.webp");
    println!("Encryption complete. Data length: {}", encrypted_image_data.len());
    //send to client encrypted image 

    fs::remove_file("hidden_message.png").unwrap();

    // Send the image data back to the client in chunks.
    const CHUNK_SIZE: usize = 1020;
    let mut seq_num: u32 = 0;
    for chunk in encrypted_image_data.chunks(CHUNK_SIZE) {
        let mut packet = vec![0; 4 + chunk.len()];
        packet[..4].copy_from_slice(&seq_num.to_be_bytes());
        packet[4..].copy_from_slice(chunk);

        // Send the packet with a sequence number.
        loop {
            my_socket.send_to(&packet, &client_socket_addr).await?;
            println!("Sent packet with sequence number: {}", seq_num);

            // Wait for an ACK from the client.
            let mut ack_buf = [0; 4];
            match timeout(Duration::from_secs(1), my_socket.recv_from(&mut ack_buf)).await {
                Ok(Ok((_, _))) => {
                    let ack_seq_num = u32::from_be_bytes(ack_buf);
                    if ack_seq_num == seq_num {
                        println!("Received ACK for sequence number: {}", ack_seq_num);
                        break; // Move to the next packet.
                    }
                }
                _ => {
                    println!("Timeout or error, resending sequence number: {}", seq_num);
                }
            }
        }

        seq_num += 1;
    }

    // Signal the end of transmission.
    my_socket.send_to(b"END", client_socket_addr).await?;
    println!("End of transmission signal sent to client.");
    let mut active_clients = ACTIVE_CLIENTS.lock().unwrap();
    active_clients.remove(&image_id.unique_identifier);

    Ok(())

}

async fn handle_initial_request(image_id: ImageID) {
    let mut active_clients = ACTIVE_CLIENTS.lock().unwrap();
    if active_clients.contains_key(&image_id.unique_identifier) {
        println!("Client {} is already being handled.", image_id.unique_identifier);
        return;
    }
    active_clients.insert(image_id.unique_identifier.clone(), true);
    drop(active_clients);
    
    if let Ok(permit) = THREAD_SEMAPHORE.clone().try_acquire_owned() {
        // Increment semaphore by acquiring a permit
        let image_clone = image_id.clone(); 
    
        // Release the semaphore immediately
        drop(permit);
    
        // Spawn the task
        tokio::spawn(async move {
            if let Err(e) = serve_client(image_clone).await {
                eprintln!("Error serving client: {}", e);
            }
        });
    } else {
        println!("No available threads to handle the request");
    }
}

#[tokio::main]
async fn main() {

    {
        let mut state = SERVER_STATE.lock().unwrap();
        state.down = false;  // Mark server as up
    }
    
    let addr = "127.0.0.1:8080";
    let socket = UdpSocket::bind(addr).await.unwrap(); 
    let state_clone = SERVER_STATE.clone();
    //Thread always listening to upcoming requests
    let t1= tokio::spawn(async move {
              // Mark server as busy when starting to listen
              {
                let mut state = state_clone.lock().unwrap();
                state.busy = true;
             }
    
        let mut buf = vec![0; 1024];
        loop {
            // Receive data
            let (len, src) = socket.recv_from(&mut buf).await.unwrap();
            let data = String::from_utf8_lossy(&buf[..len]).to_string();
            let parts: Vec<&str> = data.split(':').collect();
            let image_id = ImageID {
                ip_address: parts[0].to_string(),
                port_number: parts[1].to_string(),
                unique_identifier: parts[2].to_string(),
            };
            //println!("Received request from {}: {}", src, image_id);

            // Handle the request
            handle_initial_request(image_id).await;
        }
    });

    t1.await.unwrap();

    // // Prevent main from exiting immediately
    // loop {
    //     tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    // }
}
