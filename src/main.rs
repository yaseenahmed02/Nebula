// use tokio::net::UdpSocket;
// use tokio::sync::mpsc;
// use std::sync::{Arc, Mutex};
// //cleause tokio::time::error::Elapsed;
// use tokio::time::{self, Duration};
// use std::env;
// use std::error::Error;
// use sysinfo::System; // For CPU utilization calculation
// use lazy_static::lazy_static;
// use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
// use std::clone::Clone;
// use serde::{Serialize, Deserialize};
// use bincode;
// use steganography::encoder::Encoder;
// use::steganography::util::*;
// use std::fs::File;
// use std::collections::HashMap;

// #[derive(Clone,Copy,Debug)]
// struct ServerState
// {
//     down:bool,
// }

// const MAX_THREADS: i32 = 20; 
// lazy_static! {
//     static ref ACTIVE_THREADS: Mutex<i32> = Mutex::new(0);  // Initialize to 0
// }
// #[derive(Serialize, Deserialize, Debug)]
// enum MessageType {
//     Segment(Vec<u8>),  // Represents image chunk or data segment
//     Ack(i32),          // Represents an acknowledgment with a specific ID
//     Request(String),   // Represents a request message
// }
// #[derive(Serialize, Deserialize, Debug)]
// struct Message
// {
//     msgtype:MessageType,
// }
// #[derive(Clone,Debug)]
// struct ImageID {
//     ip_address: String,
//     port_number: String,
//     unique_identifier: String,
// }

// //map from image id to pipe tx



// struct Pipe {
//     image_id: ImageID,  // Contains IP, port, unique identifier
//     pipe_tx: mpsc::Sender<Message>,  
//     pipe_rx: mpsc::Receiver<Message>, 
//     socket: UdpSocket, 
// }

// pub fn encrypt_image(image_bytes: Vec<u8>, hidden_image_path: &str) -> Vec<u8> {

//     let payload = image_bytes.clone();
//     let destination_image = file_as_dynamic_image(hidden_image_path.to_string());
//     //Create an encoder
//     let enc = Encoder::new(&payload, destination_image);
//     //Encode our message into the alpha channel of the image
//     let result = enc.encode_alpha();
//     //Save the new image
//     save_image_buffer(result, "hidden_message.png".to_string());

//     let result_bytes = file_to_bytes(File::open("hidden_message.png").unwrap());

//     result_bytes
    
// }

// async fn passing_msg(message_type: MessageType, image_id: String)
// {
//     match message_type {
//         MessageType::Segment(_) | MessageType::Ack(_)  => {
//             serving_client(None, image_id, Some(message_type)).await; 
//         },

//         MessageType::Request(_) => {
//             initial_request(image_id).await;
//         },
//     }
// }

// async fn initial_request(image_id: ImageID)
// {
//     let mut active_threads = ACTIVE_THREADS.lock().unwrap();

//     if *active_threads < MAX_THREADS 
//     {
//         *active_threads += 1;
//         let (tx, rx) = mpsc::channel();
//         // let client_socket_addr: std::net::SocketAddr = format!("{}:{}", image_id.ip_address, image_id.port_number)
//         // .parse()
//         // .expect("Invalid socket address");

//         let my_socket = UdpSocket::bind("0.0.0.0:0").await.expect("Failed to bind socket");
//         let pipe = Pipe {
//             image_id: image_id,
//             pipe_tx: tx,
//             pipe_rx: rx,
//             socket:my_socket, 
//         };
//         let image_id_clone = image_id.clone();
//         tokio::spawn(async move {
//             serving_client(Some(pipe),image_id_clone).await;
//             let mut active_threads = ACTIVE_THREADS.lock().unwrap();
//             *active_threads -= 1;
//         });

        
//     }
// }
// async fn serving_client(rx:mpsc::Receiver<MessageType>, image_id: String)
// {
//     //Declare a vector to store the image (code karim)
//     let socket = UdpSocket::bind("0.0.0.0:0").await.expect("Failed to bind socket");
//     while let Some(message) = rx.recv().await
//      {
//         //inside the function if the size of the hashmap is zero, this should be the -1 ACK

//     let mut received_image_data = Vec::new();
//     let mut buf = [0; 1024];
//     let mut expected_seq_num = 0;
//     let mut client_addr: std::net::SocketAddr;

//     loop {
//         // Receive data from the client.
//         let (len, addr) = socket.recv_from(&mut buf).await?;
//         let data = &buf[..len];
//         client_addr = addr;

//         if data == b"END" {
//             println!("End of transmission received.");
//             break;
//         }

//         // Extract the sequence number.
//         if len < 4 {
//             println!("Received invalid packet.");
//             continue;
//         }
//         let seq_num = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
//         let payload = &data[4..];

// 		println!("Before: Received packet with sequence number: {}", seq_num);
//         // Check if the sequence number is as expected.
//         if seq_num == expected_seq_num {
//             println!("After: Received packet with sequence number: {}", seq_num);
//             received_image_data.extend_from_slice(payload);
//             expected_seq_num += 1;
//             socket.send_to(&seq_num.to_be_bytes(), addr).await?;
//         } else {
//             println!(
//                 "Out of order packet received. Expected: {}, got: {}. Discarding.",
//                 expected_seq_num, seq_num
//             );
//         }

//         // Send ACK for the last received sequence number.
//         // socket.send_to(&seq_num.to_be_bytes(), addr).await?;
//     }

//     // // encrypt_image(&mut received_image_data);
//     println!("Received image data. Encrypting...");
//     let encrypted_image_data = encrypt_image(received_image_data.clone(), "server.webp");
//     println!("Image data encrypted.");



    
//         // println!("Serving client for image_id: {:?}", pipe.image_id);

//         // let client_addr: std::net::SocketAddr = format!("{}:{}", pipe.image_id.ip_address, pipe.image_id.port_number)
//         // .parse()
//         // .expect("Invalid client address");

//         // let response_message = Message {
//         //     msgtype: MessageType::Ack(-1), 
//         // };
//         // let serialized_message = match bincode::serialize(&response_message) {
//         //     Ok(bytes) => bytes,
//         //     Err(e) => {
//         //         println!("Failed to serialize message: {:?}", e);
//         //         return;
//         //     }
//         // };
//         // match socket.send_to(&serialized_message, client_addr).await {
//         // Ok(bytes_sent) => println!("Sent {} bytes to client at {}", bytes_sent, client_addr),
//         // Err(e) => println!("Failed to send message to client: {:?}", e),
//     }
//     }
//     else
//      {
//         message_type
//      }


// }
// fn main() {
//     let mut pipe_data: HashMap<String, mpsc::Sender<MessageType> >= HashMap::new();


// };


use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;
use tokio::sync::Semaphore;
use serde::{Serialize, Deserialize};
use std::fs::File;
use steganography::encoder::Encoder;
use::steganography::util::*;

#[derive(Clone, Copy, Debug)]
struct ServerState {
    down: bool,
}

const MAX_THREADS: usize = 20;
lazy_static::lazy_static! {
    static ref ACTIVE_CLIENTS: Mutex<HashMap<String, bool>> = Mutex::new(HashMap::new());
    static ref THREAD_SEMAPHORE: Arc<Semaphore> = Arc::new(Semaphore::new(MAX_THREADS));
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
async fn serve_client(image_id: ImageID) {
    let ip = &image_id.ip_address;
    let port = &image_id.port_number;
    let socket_addr = format!("{}:{}", ip, port);
    let socket = UdpSocket::bind(&socket_addr)
        .await
        .expect("Failed to bind socket");

    println!("Serving client on {}", socket_addr);
    let response_message = Message {
        msgtype: MessageType::Ack(-1), 
    };

    let serialized_message = match bincode::serialize(&response_message) {
        Ok(bytes) => bytes,
        Err(e) => {
            println!("Failed to serialize message: {:?}", e);
            return;
        }
    };

    match socket.send_to(&serialized_message, socket_addr).await {
    Ok(bytes_sent) => println!("Sent {} bytes to client at {}", bytes_sent, socket_addr),
    Err(e) => println!("Failed to send message to client: {:?}", e),
    }


    let mut buf = [0; 1024];
    let mut received_image_data = Vec::new();
    let mut expected_seq_num = 0;

    loop {
        let (len, addr) = match socket.recv_from(&mut buf).await {
            Ok(res) => res,
            Err(_) => {
                println!("Failed to receive data, stopping client handling.");
                break;
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
            socket.send_to(&seq_num.to_be_bytes(), addr).await.expect("Failed to send ACK");
        } else {
            println!(
                "Out of order packet from {}. Expected: {}, got: {}.",
                addr, expected_seq_num, seq_num
            );
        }
    }

    println!("Received complete image data. Encrypting...");
    let encrypted_image = encrypt_image(received_image_data, "server.webp");
    println!("Encryption complete. Data length: {}", encrypted_image.len());

    let mut active_clients = ACTIVE_CLIENTS.lock().unwrap();
    active_clients.remove(&image_id.unique_identifier);
}

async fn handle_initial_request(image_id: ImageID) {
    let mut active_clients = ACTIVE_CLIENTS.lock().unwrap();
    if active_clients.contains_key(&image_id.unique_identifier) {
        println!("Client {} is already being handled.", image_id.unique_identifier);
        return;
    }
    active_clients.insert(image_id.unique_identifier.clone(), true);
    drop(active_clients);

    let permit = THREAD_SEMAPHORE.clone().acquire_owned().await.unwrap();
    tokio::spawn(async move {
        serve_client(image_id).await;
        drop(permit);
    });
    let image_clone = image_id.clone();
    if let Ok(permit) = THREAD_SEMAPHORE.clone().try_acquire_owned() {
        //let image_clone = image_id;
        tokio::spawn(async move {
            serve_client(image_clone).await;
            drop(permit);
        });
    } else {
        println!("No available threads to handle the request");
    }
}

#[tokio::main]
async fn main() {
    // Example ImageID
    let image_id = ImageID {
        ip_address: "127.0.0.1".to_string(),
        port_number: "8080".to_string(),
        unique_identifier: "client1".to_string(),
    };

    handle_initial_request(image_id).await;

    // Prevent main from exiting immediately
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
