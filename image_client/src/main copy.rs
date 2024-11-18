use tokio::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::error::Error;
use serde::{Serialize, Deserialize};
use bincode;
use tokio::sync::Semaphore;

const MAX_CONCURRENT_THREADS: usize = 3;

#[derive(Serialize, Deserialize, Debug)]
enum MessageType {
    Segment(Vec<u8>),
    Ack(i32),
    Request(String),
}

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    msgtype: MessageType,
}

struct ThreadState {
    is_busy: bool,
    current_image_id: Option<String>,
}

struct Client {
    thread_sockets: Arc<Mutex<Vec<UdpSocket>>>,
    server_addr: String,
    client_port: u16,
    thread_states: Arc<Mutex<Vec<ThreadState>>>,
    thread_semaphore: Arc<Semaphore>,
}

impl Client {
    async fn new(server_addr: String, client_port: u16) -> Result<Arc<Self>, Box<dyn Error>> {
        // Create separate sockets for each thread
        let mut sockets = Vec::new();
        for _ in 0..MAX_CONCURRENT_THREADS {
            let socket = UdpSocket::bind("0.0.0.0:0").await?;
            socket.connect(&server_addr).await?;
            sockets.push(socket);
        }

        let client = Arc::new(Client {
            thread_sockets: Arc::new(Mutex::new(sockets)),
            server_addr,
            client_port,
            thread_states: Arc::new(Mutex::new(vec![
                ThreadState { is_busy: false, current_image_id: None },
                ThreadState { is_busy: false, current_image_id: None },
                ThreadState { is_busy: false, current_image_id: None },
            ])),
            thread_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_THREADS)),
        });

        Client::initialize_permanent_threads(Arc::clone(&client)).await;
        Ok(client)
    }

    async fn initialize_permanent_threads(client: Arc<Client>) {
        for thread_id in 0..MAX_CONCURRENT_THREADS {
            let client_clone = Arc::clone(&client);
            
            tokio::spawn(async move {
                println!("Thread {} initialized and waiting for work", thread_id);
                loop {
                    let _permit = client_clone.thread_semaphore.acquire().await.unwrap();
                    
                    let image_id = {
                        let states = client_clone.thread_states.lock().unwrap();
                        if !states[thread_id].is_busy {
                            continue;
                        }
                        states[thread_id].current_image_id.clone()
                    };

                    if let Some(image_id) = image_id {
                        Client::process_image(Arc::clone(&client_clone), thread_id, image_id).await;
                        
                        let mut states = client_clone.thread_states.lock().unwrap();
                        states[thread_id].is_busy = false;
                        states[thread_id].current_image_id = None;
                        println!("Thread {} now available", thread_id);
                    }
                }
            });
        }
    }

    async fn process_image(client: Arc<Client>, thread_id: usize, image_id: String) {
        println!("\nThread {} starting to process image {}", thread_id, image_id);
        
        let socket = {
            let sockets = client.thread_sockets.lock().unwrap();
            sockets[thread_id].try_clone().unwrap()
        };

        // Send initial request
        let request_msg = Message {
            msgtype: MessageType::Request(image_id.clone()),
        };
        
        let serialized = match bincode::serialize(&request_msg) {
            Ok(data) => data,
            Err(e) => {
                println!("Thread {} serialization error: {}", thread_id, e);
                return;
            }
        };

        if let Err(e) = socket.send(&serialized).await {
            println!("Thread {} failed to send request: {}", thread_id, e);
            return;
        }

        // Wait for initial acknowledgment
        let mut buf = vec![0u8; 1024];
        match socket.recv(&mut buf).await {
            Ok(size) => {
                match bincode::deserialize::<Message>(&buf[..size]) {
                    Ok(response) => {
                        match response.msgtype {
                            MessageType::Ack(-1) => {
                                println!("Thread {} received initial acknowledgment for image {}", 
                                    thread_id, image_id);
                            },
                            _ => {
                                println!("Thread {} received unexpected initial response for image {}", 
                                    thread_id, image_id);
                                return;
                            }
                        }
                    },
                    Err(e) => {
                        println!("Thread {} failed to deserialize response: {}", thread_id, e);
                        return;
                    }
                }
            },
            Err(e) => {
                println!("Thread {} failed to receive acknowledgment: {}", thread_id, e);
                return;
            }
        }

        // Process image segments
        let image_data = create_sample_image();
        let segment_size = 1000;
        let segments: Vec<Vec<u8>> = image_data
            .chunks(segment_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        for (segment_id, segment) in segments.iter().enumerate() {
            let msg = Message {
                msgtype: MessageType::Segment(segment.clone()),
            };
            
            let serialized = match bincode::serialize(&msg) {
                Ok(data) => data,
                Err(e) => {
                    println!("Thread {} failed to serialize segment {}: {}", 
                        thread_id, segment_id, e);
                    return;
                }
            };

            if let Err(e) = socket.send(&serialized).await {
                println!("Thread {} failed to send segment {}: {}", 
                    thread_id, segment_id, e);
                return;
            }

            // Wait for segment acknowledgment
            let mut buf = vec![0u8; 1024];
            match socket.recv(&mut buf).await {
                Ok(size) => {
                    match bincode::deserialize::<Message>(&buf[..size]) {
                        Ok(response) => {
                            match response.msgtype {
                                MessageType::Ack(id) => {
                                    println!("Thread {} - Segment {} acknowledged for image {}", 
                                        thread_id, id, image_id);
                                },
                                _ => {
                                    println!("Thread {} received unexpected response for segment {} of image {}", 
                                        thread_id, segment_id, image_id);
                                    return;
                                }
                            }
                        },
                        Err(e) => {
                            println!("Thread {} failed to deserialize segment response: {}", 
                                thread_id, e);
                            return;
                        }
                    }
                },
                Err(e) => {
                    println!("Thread {} failed to receive segment acknowledgment: {}", 
                        thread_id, e);
                    return;
                }
            }
        }

        println!("Thread {} completed processing image {}", thread_id, image_id);
    }

    pub async fn submit_image(&self, image_id: String) -> Result<(), Box<dyn Error>> {
        loop {
            let mut states = self.thread_states.lock().unwrap();
            
            // Try to find the next available thread in round-robin fashion
            for thread_id in 0..MAX_CONCURRENT_THREADS {
                if !states[thread_id].is_busy {
                    states[thread_id].is_busy = true;
                    states[thread_id].current_image_id = Some(image_id.clone());
                    drop(states);
                    self.thread_semaphore.add_permits(1);
                    println!("Assigned image {} to thread {}", image_id, thread_id);
                    return Ok(());
                }
            }
            
            // If no thread is available, wait and try again
            drop(states);
            println!("All threads busy, waiting to process image {}", image_id);
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }
}

// Helper function to simulate image data
fn create_sample_image() -> Vec<u8> {
    vec![1; 10240] // 10KB sample image
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize client
    let server_addr = "127.0.0.1:8080".to_string();
    let client_port = 3000;
    
    let client = Client::new(server_addr, client_port).await?;

    // Give threads time to initialize
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    println!("\n=== All threads initialized ===");
    println!("=== Starting image submissions ===\n");

    // Submit all 10 images with minimal delay between submissions
    for i in 1..=10 {
        println!("\n=== Submitting image {} ===", i);
        match client.submit_image(format!("image{}", i)).await {
            Ok(_) => println!("Successfully queued image {}", i),
            Err(e) => println!("Failed to submit image {}: {}", i, e),
        }

        // Print thread states
        {
            let states = client.thread_states.lock().unwrap();
            println!("\nCurrent Thread States:");
            for (thread_id, state) in states.iter().enumerate() {
                println!("Thread {}: busy={}, processing={:?}", 
                        thread_id, 
                        state.is_busy, 
                        state.current_image_id);
            }
        }

        // Small delay to allow thread assignment and prevent flooding
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    println!("\n=== All images submitted, monitoring processing ===\n");

    // Monitor until all processing is complete
    loop {
        let states = client.thread_states.lock().unwrap();
        let busy_count = states.iter().filter(|state| state.is_busy).count();
        
        if busy_count > 0 {
            println!("\nProcessing Status:");
            println!("Active threads: {}/{}", busy_count, MAX_CONCURRENT_THREADS);
            for (thread_id, state) in states.iter().enumerate() {
                if state.is_busy {
                    println!("Thread {} processing: {:?}", thread_id, state.current_image_id);
                }
            }
        } else {
            println!("\n=== All images processed successfully ===");
            break;
        }
        
        drop(states);
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }

    // Final status report
    println!("\nProcessing Summary:");
    let states = client.thread_states.lock().unwrap();
    for (thread_id, state) in states.iter().enumerate() {
        println!("Thread {}: Final State - busy={}, processing={:?}", 
                thread_id, 
                state.is_busy, 
                state.current_image_id);
    }

    Ok(())
}