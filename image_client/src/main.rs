use tokio::net::UdpSocket;
use std::sync::{Arc, Mutex, mpsc};
use std::error::Error;
use serde::{Serialize, Deserialize};
use bincode;
use tokio::sync::Semaphore;

const MAX_CONCURRENT_THREADS: usize = 3;  // Maximum of 3 threads as per specification

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

// Thread state to manage the three permanent threads
struct ThreadState {
    is_busy: bool,
    current_image_id: Option<String>,
}

struct Client {
    socket: UdpSocket,
    server_addr: String,
    client_port: u16,
    thread_states: Arc<Mutex<Vec<ThreadState>>>,
    thread_semaphore: Arc<Semaphore>,
}

impl Client {
    async fn new(server_addr: String, client_port: u16) -> Result<Arc<Self>, Box<dyn Error>> {
        let bind_addr = format!("0.0.0.0:{}", client_port);
        let socket = UdpSocket::bind(&bind_addr).await?;
        
        // Initialize three thread states
        let thread_states = Arc::new(Mutex::new(vec![
            ThreadState { is_busy: false, current_image_id: None },
            ThreadState { is_busy: false, current_image_id: None },
            ThreadState { is_busy: false, current_image_id: None },
        ]));

        let client = Arc::new(Client {
            socket,
            server_addr,
            client_port,
            thread_states,
            thread_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_THREADS)),
        });

        // Initialize the three permanent threads
        Client::initialize_permanent_threads(Arc::clone(&client)).await;

        Ok(client)
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
                        println!("\nThread {} starting to process image {}", thread_id, image_id);
                        Client::process_image(Arc::clone(&client_clone), image_id).await;
                        
                        let mut states = client_clone.thread_states.lock().unwrap();
                        states[thread_id].is_busy = false;
                        states[thread_id].current_image_id = None;
                        println!("Thread {} now available", thread_id);
                    }
                }
            });
        }
    }
    

    async fn process_image(client: Arc<Client>, image_id: String) {
        println!("Processing image: {}", image_id);
        
        // Send initial request
        let request_msg = Message {
            msgtype: MessageType::Request(image_id.clone()),
        };
        
        let serialized = bincode::serialize(&request_msg).unwrap();
        client.socket.send_to(&serialized, &client.server_addr).await.unwrap();
        
        // Wait for initial acknowledgment
        let mut buf = vec![0u8; 1024];
        let (_, _) = client.socket.recv_from(&mut buf).await.unwrap();
        
        let response: Message = bincode::deserialize(&buf[..]).unwrap();
        match response.msgtype {
            MessageType::Ack(-1) => {
                println!("Received initial acknowledgment for image: {}", image_id);
                // Continue with image processing
            },
            _ => {
                println!("Unexpected response type for image: {}", image_id);
                return;
            }
        }

        // Simulate image data (in real implementation, this would be actual image data)
        let image_data = create_sample_image();
        
        // Process image segments
        let segment_size = 1000;
        let segments: Vec<Vec<u8>> = image_data
            .chunks(segment_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        for (segment_id, segment) in segments.iter().enumerate() {
            let msg = Message {
                msgtype: MessageType::Segment(segment.clone()),
            };
            
            let serialized = bincode::serialize(&msg).unwrap();
            client.socket.send_to(&serialized, &client.server_addr).await.unwrap();
            
            // Wait for acknowledgment
            let mut buf = vec![0u8; 1024];
            let (_, _) = client.socket.recv_from(&mut buf).await.unwrap();
            
            let response: Message = bincode::deserialize(&buf[..]).unwrap();
            match response.msgtype {
                MessageType::Ack(id) => {
                    println!("Segment {} acknowledged for image: {}", id, image_id);
                },
                _ => {
                    println!("Unexpected response for segment {} of image: {}", segment_id, image_id);
                    return;
                }
            }
        }

        println!("Completed processing image: {}", image_id);
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

        // Very small delay to allow thread assignment
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    println!("\n=== All images submitted, monitoring processing ===\n");

    // Monitor until all processing is complete
    loop {
        let states = client.thread_states.lock().unwrap();
        let busy_count = states.iter().filter(|state| state.is_busy).count();
        
        if busy_count > 0 {
            println!("\nActive threads: {}/{}", busy_count, MAX_CONCURRENT_THREADS);
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

    Ok(())
}

// Additional utility functions for better error handling and monitoring
impl Client {
    pub async fn get_thread_status(&self) -> Vec<ThreadStatus> {
        let states = self.thread_states.lock().unwrap();
        states
            .iter()
            .enumerate()
            .map(|(id, state)| ThreadStatus {
                thread_id: id,
                is_busy: state.is_busy,
                current_image: state.current_image_id.clone(),
            })
            .collect()
    }

    pub async fn wait_for_completion(&self) -> Result<(), Box<dyn Error>> {
        loop {
            let states = self.thread_states.lock().unwrap();
            if states.iter().all(|state| !state.is_busy) {
                break;
            }
            drop(states);
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ThreadStatus {
    thread_id: usize,
    is_busy: bool,
    current_image: Option<String>,
}