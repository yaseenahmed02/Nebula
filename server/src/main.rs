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
    let servers_down_count: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));

    // Start servers
    for server_id in 1..=total_servers {
        let server_status_clone = server_status.clone();
        let current_leader_clone = current_leader.clone();
        let request_in_progress_clone = request_in_progress.clone();
        let expected_seq_nums_clone = expected_seq_nums.clone();
        let servers_down_count_clone = servers_down_count.clone();

        tokio::spawn(async move {
            // Initialize middleware
            let server_status_clone_mw = server_status_clone.clone();
            middleware(server_status_clone_mw, server_id).await;

            // Start fault tolerance thread
            let server_status_clone_ft = server_status_clone.clone();
            let request_in_progress_clone_ft = request_in_progress_clone.clone();
            let servers_down_count_clone_ft = servers_down_count_clone.clone();
            let server_id_clone_ft = server_id;
            thread::spawn(move || {
                fault_tolerance_thread(
                    server_status_clone_ft,
                    server_id_clone_ft,
                    request_in_progress_clone_ft,
                    servers_down_count_clone_ft,
                );
            });

            // Start load balancing thread
            let server_status_clone_lb = server_status_clone.clone();
            let current_leader_clone_lb = current_leader_clone.clone();
            let request_in_progress_clone_lb = request_in_progress_clone.clone();
            let expected_seq_nums_clone_lb = expected_seq_nums_clone.clone();
            let server_id_clone_lb = server_id;
            thread::spawn(move || {
                load_balancing_thread(
                    server_status_clone_lb,
                    current_leader_clone_lb,
                    server_id_clone_lb,
                    total_servers,
                    request_in_progress_clone_lb,
                    expected_seq_nums_clone_lb,
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
                        if let Err(e) = handle_request(
                            &socket,
                            server_id,
                            &request_in_progress_clone,
                            &expected_seq_nums_clone,
                        )
                        .await
                        {
                            eprintln!("Error in handle_request: {}", e);
                        }
                    } else {
                        if let Err(e) =
                            respond_to_leader_query(&socket, &current_leader_clone).await
                        {
                            eprintln!("Error in respond_to_leader_query: {}", e);
                        }
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
    loop {
        // Wait for a packet
        let mut received_image_data = Vec::new();
        let mut buf = [0; 1024];

        let (mut len, mut addr) = socket.recv_from(&mut buf).await?;
        let mut data = &buf[..len];

        if data == b"LEADER" {
            // Handle leader query if needed
            continue;
        }

        if data == b"END" {
            // Should not happen here
            println!(
                "Unexpected END received before starting a request from {}",
                addr
            );
            continue;
        }

        if len < 4 {
            println!("Received invalid packet.");
            continue;
        }

        // Set request in progress only when we start handling a valid request
        {
            let mut flag_map = request_in_progress.lock().unwrap();
            flag_map.insert(server_id, true);
        }
        println!(
            "Server {} (Leader) started processing client request...",
            server_id
        );

        // Initialize client address and key
        let client_addr = addr;
        let client_key = addr.to_string();
        let client_key_ref = &client_key;

        // Begin processing the client request
        loop {
            // Process the packet
            if data == b"END" {
                println!("End of transmission received from client {}", addr);
                break;
            }

            if len < 4 {
                println!("Received invalid packet.");
                // Receive the next packet
                let (len_new, addr_new) = socket.recv_from(&mut buf).await?;
                len = len_new;
                addr = addr_new;
                data = &buf[..len];
                continue;
            }

            let seq_num = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            let payload = &data[4..];

            {
                let mut seq_nums = expected_seq_nums.lock().unwrap();
                let expected_seq_num = seq_nums.entry(client_key_ref.clone()).or_insert(0);

                if seq_num == *expected_seq_num {
                    println!(
                        "Leader {} received packet with sequence number: {}",
                        server_id, seq_num
                    );
                    received_image_data.extend_from_slice(payload);
                    *expected_seq_num += 1;
                } else {
                    println!(
                        "Out of order packet received from {}. Expected: {}, got: {}. Discarding.",
                        client_key_ref, *expected_seq_num, seq_num
                    );
                }
            }

            // Send ACK for the received packet regardless of whether it was expected
            socket.send_to(&seq_num.to_be_bytes(), addr).await?;

            // Receive the next packet
            let (len_new, addr_new) = socket.recv_from(&mut buf).await?;
            len = len_new;
            addr = addr_new;
            data = &buf[..len];
        }

        // Remove client's expected_seq_num
        {
            let mut seq_nums = expected_seq_nums.lock().unwrap();
            seq_nums.remove(client_key_ref);
        }

        // Processing complete, reset request_in_progress
        {
            let mut flag_map = request_in_progress.lock().unwrap();
            flag_map.insert(server_id, false);
        }

        println!("Received image data. Encrypting...");
        let image_path = "../encrypt/encryption_img.jpg".to_string(); // Update this path as needed
        let encrypted_image_data = encrypt_image(received_image_data.clone(), &image_path);
        println!("Image data encrypted.");

        // Send encrypted data back to the client
        const CHUNK_SIZE: usize = 1020;
        let mut seq_num: u32 = 0;

        for chunk in encrypted_image_data.chunks(CHUNK_SIZE) {
            let mut packet = vec![0; 4 + chunk.len()];
            packet[..4].copy_from_slice(&seq_num.to_be_bytes());
            packet[4..].copy_from_slice(chunk);

            loop {
                socket.send_to(&packet, client_addr).await?;
                println!("Sent packet with sequence number: {}", seq_num);

                let mut ack_buf = [0; 4];
                match timeout(Duration::from_secs(1), socket.recv_from(&mut ack_buf)).await {
                    Ok(Ok((len_ack, _))) => {
                        if len_ack == 4 {
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
                        } else {
                            println!("Received invalid ACK length.");
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

        // Break to wait for the next client request
        break;
    }

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
    servers_down_count: Arc<Mutex<u32>>,
) {
    loop {
        let mut rng = rand::thread_rng();
        let simulate_failure = rng.gen_bool(0.05); // Reduced failure probability to 5%

        if simulate_failure {
            // Check if only one server is down
            let mut down_count = servers_down_count.lock().unwrap();
            if *down_count >= 1 {
                // Another server is already down
                drop(down_count);
                thread::sleep(Duration::from_secs(10)); // Wait before trying again
                continue;
            }

            // Proceed to simulate failure
            let request_flag = {
                let flag_map = request_in_progress.lock().unwrap();
                *flag_map.get(&server_id).unwrap_or(&false)
            };
            if request_flag {
                println!(
                    "Server {} is processing a request, delaying failure simulation...",
                    server_id
                );
                drop(down_count);
                thread::sleep(Duration::from_secs(10));
                continue;
            }

            {
                let mut status = server_status.lock().unwrap();
                println!("Simulating failure on server {}", server_id);
                status.insert(server_id, false);
            }

            // Increment servers_down_count
            *down_count += 1;
            drop(down_count);

            thread::sleep(Duration::from_secs(15)); // Failure duration

            {
                let mut status = server_status.lock().unwrap();
                status.insert(server_id, true);
                println!("Server {} is back UP", server_id);
            }

            // Decrement servers_down_count
            let mut down_count = servers_down_count.lock().unwrap();
            *down_count -= 1;
            drop(down_count);
        } else {
            thread::sleep(Duration::from_secs(10)); // Increased sleep duration
        }
    }
}

fn load_balancing_thread(
    server_status: ServerStatus,
    current_leader: Leader,
    server_id: u32,
    total_servers: u32,
    request_in_progress: RequestInProgress,
    expected_seq_nums: ExpectedSeqNums,
) {
    loop {
        {
            let is_leader = {
                let leader = current_leader.lock().unwrap();
                *leader == server_id
            };

            if !is_leader {
                // Only the leader handles rotation
                thread::sleep(Duration::from_secs(10));
                continue;
            }

            let request_flag = {
                let flag_map = request_in_progress.lock().unwrap();
                *flag_map.get(&server_id).unwrap_or(&false)
            };

            // Delay rotation if the current leader is handling a request
            if request_flag {
                println!("Leader is busy handling a request; delaying leader rotation.");
                thread::sleep(Duration::from_secs(5));
                continue;
            }

            // Proceed to rotate the leader
            let mut leader = current_leader.lock().unwrap();

            // Find the next available server
            let status = server_status.lock().unwrap();
            let mut next_leader = *leader;
            for _ in 0..total_servers {
                next_leader = (next_leader % total_servers) + 1;
                if *status.get(&next_leader).unwrap_or(&false) && next_leader != *leader {
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
                println!("Leader rotated to server {}", *leader);

                // Clear expected_seq_nums when leader changes
                let mut seq_nums = expected_seq_nums.lock().unwrap();
                seq_nums.clear();
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
        // Send leader information in response to unexpected data
        let response = format!("NOT_LEADER {}", leader_addr);
        socket.send_to(response.as_bytes(), addr).await?;
        println!(
            "Non-leader server informed client {} about the current leader",
            addr
        );
    }

    Ok(())
}
