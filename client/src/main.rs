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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // List of server addresses for multicasting
    let server_addresses = vec![
        "127.0.0.1:8080",
        "127.0.0.1:8081",
        "127.0.0.1:8082",
    ];

    // Bind to an ephemeral local address
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    let image_path = Path::new("/home/yaseen/Nebula/client/client.jpg");
    let image_data = fs::read(image_path).await?;
    println!("Image size: {} bytes", image_data.len());

    const CHUNK_SIZE: usize = 1020;
    let mut seq_num: u32 = 0;

    // Send image data to all servers
    for chunk in image_data.chunks(CHUNK_SIZE) {
        let mut packet = vec![0; 4 + chunk.len()];
        packet[..4].copy_from_slice(&seq_num.to_be_bytes());
        packet[4..].copy_from_slice(chunk);

        for server_addr in &server_addresses {
            if let Err(e) = socket.send_to(&packet, server_addr).await {
                eprintln!("Error sending packet {} to {}: {}", seq_num, server_addr, e);
            } else {
                println!("Sent packet {} to server {}", seq_num, server_addr);
            }
        }

        // Wait for an ACK from the leader server
        let mut ack_received = false;
        loop {
            let mut ack_buf = [0; 4];
            match timeout(Duration::from_secs(2), socket.recv_from(&mut ack_buf)).await {
                Ok(Ok((_, addr))) => {
                    let ack_seq_num = u32::from_be_bytes(ack_buf);
                    if ack_seq_num == seq_num {
                        println!("Received ACK for sequence {} from server {}", ack_seq_num, addr);
                        ack_received = true;
                        break;
                    } else {
                        println!("Received out-of-sequence ACK for {} from {}", ack_seq_num, addr);
                    }
                }
                _ => {
                    println!("Timeout or error, resending packet {}", seq_num);
                    for server_addr in &server_addresses {
                        if let Err(e) = socket.send_to(&packet, server_addr).await {
                            eprintln!("Error resending packet to {}: {}", server_addr, e);
                        }
                    }
                }
            }

            if ack_received {
                break;
            }
        }

        seq_num += 1;
    }

    // Signal the end of transmission to all servers
    for server_addr in &server_addresses {
        if let Err(e) = socket.send_to(b"END", server_addr).await {
            eprintln!("Error sending END signal to {}: {}", server_addr, e);
        } else {
            println!("End of transmission signal sent to server {}", server_addr);
        }
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

    Ok(())
}
