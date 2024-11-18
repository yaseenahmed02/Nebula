use tokio::net::UdpSocket;
use std::sync::{Arc, Mutex,mpsc};
//cleause tokio::time::error::Elapsed;
use tokio::time::{self, Duration};
use std::env;
use std::error::Error;
use sysinfo::System; // For CPU utilization calculation
use lazy_static::lazy_static;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use std::clone::Clone;
use serde::{Serialize, Deserialize};
use bincode;

#[derive(Clone,Copy,Debug)]
struct ServerState
{
    down:bool,
}

const MAX_THREADS: i32 = 20; 
lazy_static! {
    static ref ACTIVE_THREADS: Mutex<i32> = Mutex::new(0);  // Initialize to 0
}
#[derive(Serialize, Deserialize, Debug)]enum MessageType {
    Segment(Vec<u8>),  // Represents image chunk or data segment
    Ack(i32),          // Represents an acknowledgment with a specific ID
    Request(String),   // Represents a request message
}
#[derive(Serialize, Deserialize, Debug)]
struct Message
{
    msgtype:MessageType,
}
#[derive(Clone,Debug)]
struct ImageID {
    ip_address: String,
    port_number: String,
    unique_identifier: String,
}
struct Pipe {
    image_id: ImageID,  // Contains IP, port, unique identifier
    pipe_tx: mpsc::Sender<Message>,  
    pipe_rx: mpsc::Receiver<Message>, 
    socket: UdpSocket, 
}

async fn passing_msg(message_type: MessageType, image_id: ImageID)
{
    match message_type {
        MessageType::Segment(_) | MessageType::Ack(_)  => {
            serving_client(None, image_id).await; 
        },

        MessageType::Request(_) => {
            initial_request(image_id).await;
        },
    }
}

async fn initial_request(image_id: ImageID)
{
    let mut active_threads = ACTIVE_THREADS.lock().unwrap();

    if *active_threads < MAX_THREADS 
    {
        *active_threads += 1;
        let (tx, rx) = mpsc::channel();
        // let client_socket_addr: std::net::SocketAddr = format!("{}:{}", image_id.ip_address, image_id.port_number)
        // .parse()
        // .expect("Invalid socket address");

        let my_socket = UdpSocket::bind("0.0.0.0:0").await.expect("Failed to bind socket");
        let pipe = Pipe {
            image_id: image_id.clone(),
            pipe_tx: tx,
            pipe_rx: rx,
            socket:my_socket, 
        };
        let image_id_clone = image_id.clone();
        tokio::spawn(async move {
            serving_client(Some(pipe),image_id_clone).await;
            let mut active_threads = ACTIVE_THREADS.lock().unwrap();
            *active_threads -= 1;
        });

        
    }
}
async fn serving_client(pipe: Option<Pipe>, image_id: ImageID)
{
    let socket = UdpSocket::bind("0.0.0.0:0").await.expect("Failed to bind socket");
    if let Some(mut pipe) = pipe
     {
        println!("Serving client for image_id: {:?}", pipe.image_id);

        let client_addr: std::net::SocketAddr = format!("{}:{}", pipe.image_id.ip_address, pipe.image_id.port_number)
        .parse()
        .expect("Invalid client address");

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

        match socket.send_to(&serialized_message, client_addr).await {
        Ok(bytes_sent) => println!("Sent {} bytes to client at {}", bytes_sent, client_addr),
        Err(e) => println!("Failed to send message to client: {:?}", e),
    }
    }
    else
     {
        
     }


}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "127.0.0.1:8080";
    let socket = UdpSocket::bind(addr).await?;
    println!("Server listening on {}", addr);

    let mut buf = vec![0u8; 1024];
    loop {
        let (size, src_addr) = socket.recv_from(&mut buf).await?;
        let message: Message = bincode::deserialize(&buf[..size])?;

        let image_id = ImageID {
            ip_address: src_addr.ip().to_string(),
            port_number: src_addr.port().to_string(),
            unique_identifier: "unique_id".to_string(),
        };

        match message.msgtype {
            MessageType::Request(_) => {
                initial_request(image_id).await;
            },
            MessageType::Segment(data) => {
                // Handle segment
                let response = Message {
                    msgtype: MessageType::Ack(1),
                };
                let serialized = bincode::serialize(&response)?;
                socket.send_to(&serialized, src_addr).await?;
            },
            _ => {}
        }
    }
}