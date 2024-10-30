use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};
use tokio::fs;
use std::{error::Error, fs::File};
use std::path::Path;
use steganography::util::{bytes_to_file, file_as_image_buffer};
use steganography::decoder::Decoder;

fn decrypt_image() {
    let encoded_image = file_as_image_buffer("hidden_message.png".to_string());
    let dec = Decoder::new(encoded_image);
    let out_buffer = dec.decode_alpha();
    bytes_to_file(&out_buffer, &File::create("decoded_image.png").unwrap());
}

async fn discover_leader(server_addresses: &[&str], socket: &UdpSocket) -> Option<String> {
    for server_addr in server_addresses {
        // Send a "LEADER" query to each server
        if let Err(e) = socket.send_to(b"LEADER", server_addr).await {
            eprintln!("Error querying leader at {}: {}", server_addr, e);
            continue;
        }

        let mut buf = [0; 1024];
        match timeout(Duration::from_secs(1), socket.recv_from(&mut buf)).await {
            Ok(Ok((len, _))) => {
                let response = String::from_utf8_lossy(&buf[..len]);
                if response.starts_with("LEADER") {
                    // The server responded with the leader's address
                    let leader_addr = response.trim_start_matches("LEADER ").trim().to_string();
                    println!("Discovered leader: {}", leader_addr);
                    return Some(leader_addr);
                }
            }
            _ => {
                eprintln!("No response from server {}", server_addr);
            }
        }
    }
    None
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // List of server addresses
    let server_addresses = vec![
        "127.0.0.1:8081",
        "127.0.0.1:8082",
        "127.0.0.1:8083",
    ];

    // Bind to an ephemeral local address
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    let image_path = Path::new("/home/yaseen/Nebula/client/client.jpg");

    loop {
        // Prepare image data for sending
        let image_data = fs::read(image_path).await?;
        println!("Image size: {} bytes", image_data.len());

        const CHUNK_SIZE: usize = 1020;
        let mut seq_num: u32 = 0; // Reset sequence number

        // Discover the leader
        let mut leader_addr = match discover_leader(&server_addresses, &socket).await {
            Some(addr) => addr,
            None => {
                eprintln!("Failed to discover leader");
                return Ok(());
            }
        };

        // Send image data to the leader
        for chunk in image_data.chunks(CHUNK_SIZE) {
            let mut packet = vec![0; 4 + chunk.len()];
            packet[..4].copy_from_slice(&seq_num.to_be_bytes());
            packet[4..].copy_from_slice(chunk);

            let mut retry_count = 0;
            const MAX_RETRIES: u32 = 5;

            loop {
                if let Err(e) = socket.send_to(&packet, &leader_addr).await {
                    eprintln!("Error sending packet {} to leader {}: {}", seq_num, leader_addr, e);
                } else {
                    println!("Sent packet {} to leader {}", seq_num, leader_addr);
                }

                // Wait for an ACK from the leader
                let mut ack_buf = [0; 4];
                match timeout(Duration::from_secs(2), socket.recv_from(&mut ack_buf)).await {
                    Ok(Ok((_, addr))) => {
                        let ack_seq_num = u32::from_be_bytes(ack_buf);
                        if ack_seq_num == seq_num {
                            println!("Received ACK for sequence {} from leader {}", ack_seq_num, addr);
                            break;
                        } else {
                            println!("Received out-of-sequence ACK for {} from {}", ack_seq_num, addr);
                        }
                    }
                    _ => {
                        println!("Timeout or error, resending packet {}", seq_num);
                        retry_count += 1;
                        if retry_count >= MAX_RETRIES {
                            println!("Max retries reached, rediscovering leader...");
                            if let Some(new_leader) = discover_leader(&server_addresses, &socket).await {
                                leader_addr = new_leader;
                                retry_count = 0;
                            } else {
                                eprintln!("Failed to rediscover leader");
                                return Ok(());
                            }
                        }
                    }
                }
            }

            seq_num += 1;
        }

        // Signal the end of transmission to the leader
        if let Err(e) = socket.send_to(b"END", &leader_addr).await {
            eprintln!("Error sending END signal to leader {}: {}", leader_addr, e);
        } else {
            println!("End of transmission signal sent to leader {}", leader_addr);
        }

        // Receive the encrypted image data back from the leader server
        let mut received_image_data = Vec::new();
        let mut buf = [0; 1024];

        loop {
            let (len, addr) = socket.recv_from(&mut buf).await?;
            if &buf[..len] == b"END" {
                println!("End of transmission received from server {}", addr);
                break;
            }

            // Extract the sequence number from the received packet
            if len < 4 {
                eprintln!("Received invalid packet from server {}", addr);
                continue;
            }
            let seq_num = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
            let payload = &buf[4..len];
            println!("Received packet {} from server {}", seq_num, addr);

            received_image_data.extend_from_slice(payload);

            // Send ACK for the received packet
            if let Err(e) = socket.send_to(&seq_num.to_be_bytes(), addr).await {
                eprintln!("Error sending ACK for packet {}: {}", seq_num, e);
            }
        }

        // Save and decrypt the received image
        bytes_to_file(&received_image_data, &File::create("hidden_message.png")?);
        println!("Received encrypted image saved as hidden_message.png.");
        decrypt_image();
        println!("Decrypted image saved as decoded_image.png.");

        // Decide whether to send another request or exit
        // For now, let's break the loop
        break;
    }

    Ok(())
}
