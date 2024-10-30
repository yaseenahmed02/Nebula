use encrypt::encode::encrypt_image;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::net::UdpSocket;
use tokio::time::{sleep, timeout, Duration};
use rand::Rng;

type ServerStatus = Arc<Mutex<HashMap<u32, bool>>>;
type Leader = Arc<Mutex<u32>>;
type RequestInProgress = Arc<Mutex<bool>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let server_id = 1; // Unique ID for each server
    let total_servers = 3;

    let server_status: ServerStatus = Arc::new(Mutex::new(HashMap::new()));
    let current_leader: Leader = Arc::new(Mutex::new(1));
    let request_in_progress: RequestInProgress = Arc::new(Mutex::new(false));

    let server_status_clone = server_status.clone();
    tokio::spawn(async move {
        middleware(server_status_clone, server_id).await;
    });

    let server_status_clone = server_status.clone();
    thread::spawn(move || {
        fault_tolerance_thread(server_status_clone, server_id);
    });

    let server_status_clone = server_status.clone();
    let current_leader_clone = current_leader.clone();
    let request_in_progress_clone = request_in_progress.clone();
    thread::spawn(move || {
        load_balancing_thread(server_status_clone, current_leader_clone, server_id, total_servers, request_in_progress_clone);
    });

    let socket = UdpSocket::bind("127.0.0.1:8080").await?;
    println!("Server {} listening on {}", server_id, "127.0.0.1:8080");

    loop {
        let is_leader = {
            let leader = current_leader.lock().unwrap();
            *leader == server_id
        };

        let is_up = {
            let status = server_status.lock().unwrap();
            *status.get(&server_id).unwrap_or(&true)
        };

        if is_up {
            if is_leader {
                println!("Server {} (Leader) is waiting for client requests...", server_id);
                handle_request(&socket, server_id, &request_in_progress).await?;
            } else {
                forward_request_to_leader(&socket, &current_leader).await?;
            }
        } else {
            println!("Server {} is down or not the leader.", server_id);
            sleep(Duration::from_secs(2)).await;
        }
    }
}

async fn handle_request(socket: &UdpSocket, server_id: u32, request_in_progress: &RequestInProgress) -> Result<(), Box<dyn Error>> {
    let mut received_image_data = Vec::new();
    let mut buf = [0; 1024];
    let mut expected_seq_num = 0;
    let mut client_addr: std::net::SocketAddr;

    loop {
        let (len, addr) = match socket.recv_from(&mut buf).await {
            Ok((len, addr)) => (len, addr),
            Err(_) => continue, // Ignore any errors in receiving data for simplicity
        };

        let data = &buf[..len];
        client_addr = addr;

        // Set request in progress only after we start handling a valid request
        {
            let mut flag = request_in_progress.lock().unwrap();
            if !*flag {
                *flag = true;
                println!("Server {} (Leader) started processing client request...", server_id);
            }
        }

        if data == b"END" {
            println!("End of transmission received.");
            break;
        }

        if len < 4 {
            println!("Received invalid packet.");
            continue;
        }

        let seq_num = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let payload = &data[4..];

        if seq_num == expected_seq_num {
            println!("Leader {} received packet with sequence number: {}", server_id, seq_num);
            received_image_data.extend_from_slice(payload);
            expected_seq_num += 1;
        } else {
            println!(
                "Out of order packet received. Expected: {}, got: {}. Discarding.",
                expected_seq_num, seq_num
            );
        }

        socket.send_to(&seq_num.to_be_bytes(), addr).await?;
    }

    // Processing complete, reset request_in_progress
    {
        let mut flag = request_in_progress.lock().unwrap();
        *flag = false;
    }

    println!("Received image data. Encrypting...");
    let encrypted_image_data = encrypt_image(received_image_data.clone(), "/home/yaseen/Nebula/encrypt/encryption_img.jpg");
    println!("Image data encrypted.");

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
                Ok(Ok((_, _))) => {
                    let ack_seq_num = u32::from_be_bytes(ack_buf);
                    if ack_seq_num == seq_num {
                        println!("Received ACK for sequence number: {}", ack_seq_num);
                        break;
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

fn fault_tolerance_thread(server_status: ServerStatus, server_id: u32) {
    loop {
        let mut rng = rand::thread_rng();
        let mut status = server_status.lock().unwrap();
        let down_count = status.values().filter(|&&state| !state).count();

        if down_count < 1 && rng.gen_bool(0.1) {
            println!("Simulating failure on server {}", server_id);
            status.insert(server_id, false);
            drop(status);

            thread::sleep(Duration::from_secs(10));

            let mut status = server_status.lock().unwrap();
            status.insert(server_id, true);
            println!("Server {} is back UP", server_id);
        }

        thread::sleep(Duration::from_secs(5));
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
        // Only rotate if no request is in progress.
        {
            let mut leader = current_leader.lock().unwrap();
            let mut request_flag = request_in_progress.lock().unwrap();

            // Delay rotation if the current leader is handling a request
            if *request_flag {
                println!("Leader busy, delaying leader rotation...");
                thread::sleep(Duration::from_secs(5));
                continue;
            }

            // Increment leader and loop back to 1 if it exceeds the number of servers
            *leader = ((*leader % total_servers) + 1) as u32;

            // Ensure the new leader is up
            let status = server_status.lock().unwrap();
            while !*status.get(&*leader).unwrap_or(&true) {
                println!("Skipping down server {} for leader rotation.", *leader);
                *leader = ((*leader % total_servers) + 1) as u32;
            }

            println!("Server {} set new leader to: {}", server_id, *leader);
        }

        // Allow time before checking for the next rotation
        thread::sleep(Duration::from_secs(10));
    }
}


async fn forward_request_to_leader(socket: &UdpSocket, current_leader: &Leader) -> Result<(), Box<dyn Error>> {
    let leader_id = *current_leader.lock().unwrap();
    let leader_addr = format!("127.0.0.1:808{}", leader_id); // Assuming each server runs on a different port (8081, 8082, etc.)
    println!("Forwarding request to leader at {}", leader_addr);

    // Forward the packet to the leader.
    let mut buf = [0; 1024];
    let (len, addr) = socket.recv_from(&mut buf).await?;
    socket.send_to(&buf[..len], &leader_addr).await?;
    println!("Request forwarded to leader by server at {}", addr);

    Ok(())
}


