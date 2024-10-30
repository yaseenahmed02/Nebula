use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};
use tokio::fs;
use std::{error::Error, fs::File, io::Write, sync::Mutex, path::Path};
use std::sync::Arc;
use steganography::util::{bytes_to_file, file_as_image_buffer};
use steganography::decoder::Decoder;

fn decrypt_image() {
    let encoded_image = file_as_image_buffer("hidden_message.png".to_string());
    let dec = Decoder::new(encoded_image);
    let out_buffer = dec.decode_alpha();
    bytes_to_file(&out_buffer, &File::create("decoded_image.png").unwrap());
}

async fn discover_leader(server_addresses: &[&str], socket: &UdpSocket, log_file: Arc<Mutex<File>>) -> Option<String> {
    for server_addr in server_addresses {
        if let Err(e) = socket.send_to(b"LEADER", server_addr).await {
            let msg = format!("Error querying leader at {}: {}\n", server_addr, e);
            log(log_file.clone(), &msg);
            continue;
        }

        let mut buf = [0; 1024];
        match timeout(Duration::from_secs(1), socket.recv_from(&mut buf)).await {
            Ok(Ok((len, _))) => {
                let response = String::from_utf8_lossy(&buf[..len]);
                if response.starts_with("LEADER") {
                    let leader_addr = response.trim_start_matches("LEADER ").trim().to_string();
                    let msg = format!("Discovered leader: {}\n", leader_addr);
                    log(log_file.clone(), &msg);
                    return Some(leader_addr);
                }
            }
            _ => {
                let msg = format!("No response from server {}\n", server_addr);
                log(log_file.clone(), &msg);
            }
        }
    }
    None
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let log_file = Arc::new(Mutex::new(File::create("client_log.txt")?));

    let server_addresses = vec!["127.0.0.1:8081", "127.0.0.1:8082", "127.0.0.1:8083"];
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    let image_path = Path::new("client.jpg");

    loop {
        let image_data = fs::read(image_path).await?;
        log(log_file.clone(), &format!("Image size: {} bytes\n", image_data.len()));

        const CHUNK_SIZE: usize = 1020;
        let mut seq_num: u32 = 0;

        let mut leader_addr = match discover_leader(&server_addresses, &socket, log_file.clone()).await {
            Some(addr) => addr,
            None => {
                log(log_file.clone(), "Failed to discover leader\n");
                return Ok(());
            }
        };

        for chunk in image_data.chunks(CHUNK_SIZE) {
            let mut packet = vec![0; 4 + chunk.len()];
            packet[..4].copy_from_slice(&seq_num.to_be_bytes());
            packet[4..].copy_from_slice(chunk);

            let mut retry_count = 0;
            const MAX_RETRIES: u32 = 3;

            loop {
                if let Err(e) = socket.send_to(&packet, &leader_addr).await {
                    let msg = format!("Error sending packet {} to leader {}: {}\n", seq_num, leader_addr, e);
                    log(log_file.clone(), &msg);
                } else {
                    let msg = format!("Sent packet {} to leader {}\n", seq_num, leader_addr);
                    log(log_file.clone(), &msg);
                }

                let mut ack_buf = [0; 1024];
                match timeout(Duration::from_secs(2), socket.recv_from(&mut ack_buf)).await {
                    Ok(Ok((len, addr))) => {
                        let response = String::from_utf8_lossy(&ack_buf[..len]);
                        if response.starts_with("NOT_LEADER") {
                            leader_addr = response.trim_start_matches("NOT_LEADER ").trim().to_string();
                            log(log_file.clone(), &format!("New leader discovered: {}\n", leader_addr));
                            retry_count = 0;
                            continue;
                        } else if len == 4 {
                            let ack_seq_num = u32::from_be_bytes(ack_buf[..4].try_into().unwrap());
                            if ack_seq_num == seq_num {
                                let msg = format!("Received ACK for sequence {} from {}\n", ack_seq_num, addr);
                                log(log_file.clone(), &msg);
                                break;
                            }
                        }
                    }
                    _ => {
                        log(log_file.clone(), &format!("Timeout, resending packet {}\n", seq_num));
                        retry_count += 1;
                        if retry_count >= MAX_RETRIES {
                            log(log_file.clone(), "Rediscovering leader after max retries\n");
                            if let Some(new_leader) = discover_leader(&server_addresses, &socket, log_file.clone()).await {
                                leader_addr = new_leader;
                                retry_count = 0;
                            } else {
                                log(log_file.clone(), "Failed to rediscover leader\n");
                                return Ok(());
                            }
                        }
                    }
                }
            }

            seq_num += 1;
        }

        if let Err(e) = socket.send_to(b"END", &leader_addr).await {
            log(log_file.clone(), &format!("Error sending END signal to leader {}: {}\n", leader_addr, e));
        } else {
            log(log_file.clone(), &format!("End of transmission signal sent to leader {}\n", leader_addr));
        }

        let mut received_image_data = Vec::new();
        let mut buf = [0; 1024];
        let mut expected_seq_num: u32 = 0;

        loop {
            let (len, addr) = socket.recv_from(&mut buf).await?;
            if &buf[..len] == b"END" {
                log(log_file.clone(), &format!("End of transmission received from server {}\n", addr));
                break;
            }

            let seq_num_received = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
            let payload = &buf[4..len];

            if seq_num_received == expected_seq_num {
                log(log_file.clone(), &format!("Received packet {} from server {}\n", seq_num_received, addr));
                received_image_data.extend_from_slice(payload);
                expected_seq_num += 1;
            } else {
                log(log_file.clone(), &format!("Out of order packet: expected {}, got {}\n", expected_seq_num, seq_num_received));
                continue;
            }

            if let Err(e) = socket.send_to(&seq_num_received.to_be_bytes(), addr).await {
                log(log_file.clone(), &format!("Error sending ACK for packet {}: {}\n", seq_num_received, e));
            }
        }

        bytes_to_file(&received_image_data, &File::create("hidden_message.png")?);
        log(log_file.clone(), "Received encrypted image saved as hidden_message.png.\n");
        decrypt_image();
        log(log_file.clone(), "Decrypted image saved as decoded_image.png.\n");

        break;
    }

    Ok(())
}

fn log(log_file: Arc<Mutex<File>>, message: &str) {
    {
        let mut file = log_file.lock().unwrap();
        file.write_all(message.as_bytes()).expect("Unable to write log to file");
    }
    print!("{}", message);
}
