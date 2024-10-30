use encrypt::encode::encrypt_image;
use rand::Rng;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::net::UdpSocket;
use tokio::time::{sleep, timeout, Duration};

type ServerStatus = Arc<Mutex<HashMap<u32, bool>>>;
type Leader = Arc<Mutex<u32>>;
type RequestInProgress = Arc<Mutex<HashMap<u32, bool>>>;
type ExpectedSeqNums = Arc<Mutex<HashMap<String, u32>>>; // Map of client_addr to expected_seq_num

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let total_servers = 3;

    let server_status: ServerStatus = Arc::new(Mutex::new(HashMap::new()));
    let current_leader: Leader = Arc::new(Mutex::new(1));
    let request_in_progress: RequestInProgress = Arc::new(Mutex::new(HashMap::new()));
    let expected_seq_nums: ExpectedSeqNums = Arc::new(Mutex::new(HashMap::new()));

    // Start servers
    for server_id in 1..=total_servers {
        let server_status_clone = server_status.clone();
        let current_leader_clone = current_leader.clone();
        let request_in_progress_clone = request_in_progress.clone();
        let expected_seq_nums_clone = expected_seq_nums.clone();

        tokio::spawn(async move {
            // Initialize middleware
            let server_status_clone_mw = server_status_clone.clone();
            middleware(server_status_clone_mw, server_id).await;

            // Start fault tolerance thread
            let server_status_clone_ft = server_status_clone.clone();
            let request_in_progress_clone_ft = request_in_progress_clone.clone();
            let server_id_clone_ft = server_id;
            thread::spawn(move || {
                fault_tolerance_thread(
                    server_status_clone_ft,
                    server_id_clone_ft,
                    request_in_progress_clone_ft,
                );
            });

            // Start load balancing thread
            let server_status_clone_lb = server_status_clone.clone();
            let current_leader_clone_lb = current_leader_clone.clone();
            let request_in_progress_clone_lb = request_in_progress_clone.clone();
            let server_id_clone_lb = server_id;
            thread::spawn(move || {
                load_balancing_thread(
                    server_status_clone_lb,
                    current_leader_clone_lb,
                    server_id_clone_lb,
                    total_servers,
                    request_in_progress_clone_lb,
                );
            });

            // Bind the server socket
            let bind_address = format!("127.0.0.1:808{}", server_id);
            let socket = UdpSocket::bind(&bind_address).await.unwrap();
            println!("Server {} listening on {}", server_id, bind_address);

            loop {
                let is_leader = {
                    let leader = current_leader_clone.lock().unwrap();
                    *leader == server_id
                };

                let is_up = {
                    let status = server_status_clone.lock().unwrap();
                    *status.get(&server_id).unwrap_or(&true)
                };

                if is_up {
                    if is_leader {
                        println!(
                            "Server {} (Leader) is waiting for client requests...",
                            server_id
                        );
                        handle_request(
                            &socket,
                            server_id,
                            &request_in_progress_clone,
                            &expected_seq_nums_clone,
                        )
                        .await
                        .unwrap();
                    } else {
                        respond_to_leader_query(&socket, &current_leader_clone)
                            .await
                            .unwrap();
                    }
                } else {
                    println!("Server {} is down. Sleeping...", server_id);
                    sleep(Duration::from_secs(2)).await;
                }
            }
        });
    }

    // Keep the main task alive
    loop {
        sleep(Duration::from_secs(60)).await;
    }
}

async fn handle_request(
    socket: &UdpSocket,
    server_id: u32,
    request_in_progress: &RequestInProgress,
    expected_seq_nums: &ExpectedSeqNums,
) -> Result<(), Box<dyn Error>> {
    let mut received_image_data = Vec::new();
    let mut buf = [0; 1024];
    let mut client_addr: Option<std::net::SocketAddr> = None;
    let mut client_key: Option<String> = None; // Declare client_key here

    loop {
        let (len, addr) = match socket.recv_from(&mut buf).await {
            Ok((len, addr)) => (len, addr),
            Err(_) => continue, // Ignore any errors in receiving data for simplicity
        };

        let data = &buf[..len];

        // Set request in progress only after we start handling a valid request
        {
            let mut flag_map = request_in_progress.lock().unwrap();
            let flag = flag_map.entry(server_id).or_insert(false);
            if !*flag {
                *flag = true;
                println!(
                    "Server {} (Leader) started processing client request...",
                    server_id
                );
            }
        }

        // Save client address
        if client_addr.is_none() {
            client_addr = Some(addr);
        }

        // Initialize client_key if it's not set
        if client_key.is_none() {
            client_key = Some(addr.to_string());
        }

        let client_key_ref = client_key.as_ref().unwrap();

        if data == b"END" {
            println!("End of transmission received from client {}", addr);
            break;
        }

        if len < 4 {
            println!("Received invalid packet.");
            continue;
        }

        let seq_num = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let payload = &data[4..];

        let is_expected_seq_num = {
            // Begin block to limit the scope of seq_nums
            let mut seq_nums = expected_seq_nums.lock().unwrap();
            let expected_seq_num = seq_nums.entry(client_key_ref.clone()).or_insert(0);

            if seq_num == *expected_seq_num {
                println!(
                    "Leader {} received packet with sequence number: {}",
                    server_id, seq_num
                );
                received_image_data.extend_from_slice(payload);
                *expected_seq_num += 1;
                true
            } else {
                println!(
                    "Out of order packet received from {}. Expected: {}, got: {}. Discarding.",
                    client_key_ref, *expected_seq_num, seq_num
                );
                false
            }
        }; // MutexGuard `seq_nums` is dropped here

        // Send ACK for the received packet regardless of whether it was expected
        socket.send_to(&seq_num.to_be_bytes(), addr).await?;

        // If the packet was out of order, you might decide to continue or break
        // For now, we'll just continue to the next iteration
        if !is_expected_seq_num {
            continue;
        }
    }

    // Remove client's expected_seq_num
    {
        let mut seq_nums = expected_seq_nums.lock().unwrap();
        if let Some(ref key) = client_key {
            seq_nums.remove(key);
        }
    }

    // Processing complete, reset request_in_progress
    {
        let mut flag_map = request_in_progress.lock().unwrap();
        flag_map.insert(server_id, false);
    }

    println!("Received image data. Encrypting...");
    
    let encrypted_image_data = encrypt_image(received_image_data.clone(), "/home/yaseen/Nebula/encrypt/encryption_img.jpg");
    println!("Image data encrypted.");

    // Send encrypted data back to the client
    const CHUNK_SIZE: usize = 1020;
    let mut seq_num: u32 = 0;

    let client_addr = match client_addr {
        Some(addr) => addr,
        None => {
            println!("Client address not found. Cannot send encrypted data.");
            return Ok(());
        }
    };

    for chunk in encrypted_image_data.chunks(CHUNK_SIZE) {
        let mut packet = vec![0; 4 + chunk.len()];
        packet[..4].copy_from_slice(&seq_num.to_be_bytes());
        packet[4..].copy_from_slice(chunk);

        loop {
            socket.send_to(&packet, client_addr).await?;
            println!("Sent packet with sequence number: {}", seq_num);

            let mut ack_buf = [0; 4];
            match timeout(Duration::from_secs(1), socket.recv_from(&mut ack_buf)).await {
                Ok(Ok((_, _))) => {
                    let ack_seq_num = u32::from_be_bytes(ack_buf);
                    if ack_seq_num == seq_num {
                        println!("Received ACK for sequence number: {}", ack_seq_num);
                        break;
                    } else {
                        println!(
                            "Received out-of-sequence ACK: {} (expected {}). Ignoring.",
                            ack_seq_num, seq_num
                        );
                    }
                }
                _ => {
                    println!("Timeout or error, resending sequence number: {}", seq_num);
                }
            }
        }

        seq_num += 1;
    }

    socket.send_to(b"END", client_addr).await?;
    println!("End of transmission signal sent to client.");

    Ok(())
}

async fn middleware(server_status: ServerStatus, server_id: u32) {
    let mut status = server_status.lock().unwrap();
    status.insert(server_id, true);
    println!("Middleware initialized for server {}", server_id);
}

fn fault_tolerance_thread(
    server_status: ServerStatus,
    server_id: u32,
    request_in_progress: RequestInProgress,
) {
    loop {
        let mut rng = rand::thread_rng();
        let simulate_failure = rng.gen_bool(0.1);

        if simulate_failure {
            let request_flag = {
                let flag_map = request_in_progress.lock().unwrap();
                *flag_map.get(&server_id).unwrap_or(&false)
            };
            if request_flag {
                println!(
                    "Server {} is processing a request, delaying failure simulation...",
                    server_id
                );
                thread::sleep(Duration::from_secs(5));
                continue;
            }

            {
                let mut status = server_status.lock().unwrap();
                println!("Simulating failure on server {}", server_id);
                status.insert(server_id, false);
            }

            thread::sleep(Duration::from_secs(10));

            {
                let mut status = server_status.lock().unwrap();
                status.insert(server_id, true);
                println!("Server {} is back UP", server_id);
            }
        } else {
            thread::sleep(Duration::from_secs(5));
        }
    }
}

fn load_balancing_thread(
    server_status: ServerStatus,
    current_leader: Leader,
    server_id: u32,
    total_servers: u32,
    request_in_progress: RequestInProgress,
) {
    loop {
        {
            let mut leader = current_leader.lock().unwrap();
            let request_flag = {
                let flag_map = request_in_progress.lock().unwrap();
                *flag_map.get(&*leader).unwrap_or(&false)
            };

            // Delay rotation if the current leader is handling a request
            if request_flag {
                println!("Leader busy, delaying leader rotation...");
                drop(leader);
                thread::sleep(Duration::from_secs(5));
                continue;
            }

            // Find the next available server
            let status = server_status.lock().unwrap();
            let mut next_leader = *leader;
            for _ in 0..total_servers {
                next_leader = (next_leader % total_servers) + 1;
                if *status.get(&next_leader).unwrap_or(&false) {
                    break;
                }
            }
            drop(status);

            if next_leader == *leader {
                println!(
                    "No other servers are up; retaining current leader {}",
                    *leader
                );
            } else {
                *leader = next_leader;
                println!("Server {} set new leader to: {}", server_id, *leader);
            }
        }

        // Allow time before checking for the next rotation
        thread::sleep(Duration::from_secs(10));
    }
}

async fn respond_to_leader_query(
    socket: &UdpSocket,
    current_leader: &Leader,
) -> Result<(), Box<dyn Error>> {
    let leader_id = *current_leader.lock().unwrap();
    let leader_addr = format!("127.0.0.1:808{}", leader_id);

    let mut buf = [0; 1024];
    let (len, addr) = socket.recv_from(&mut buf).await?;
    let data = &buf[..len];

    if data == b"LEADER" {
        // Respond with the leader's address
        let response = format!("LEADER {}", leader_addr);
        socket.send_to(response.as_bytes(), addr).await?;
        println!("Server responded to leader query from {}", addr);
    } else {
        println!("Non-leader server received unexpected data from {}", addr);
    }

    Ok(())
}
