use encrypt::encode::encrypt_image;
use std::error::Error;
use std::fs;
use tokio::net::UdpSocket;
use tokio::time::timeout;
// use std::env;
// use tokio::task;
use tokio::time::Duration;



#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // let args: Vec<String> = env::args().collect();

    // for arg in &args {
    //     println!("Argument: {}", arg);
    // }

    // let bind_address = args
    //     .get(1)
    //     .expect("Please provide a bind address as an argument");

    println!("Starting server...");

    // Bind the server to a local address.
    let socket = UdpSocket::bind("127.0.0.1:8080").await?;
    println!("Server listening on {}", "127.0.0.1:8080");

    

    let mut received_image_data = Vec::new();
    let mut buf = [0; 1024];
    let mut expected_seq_num = 0;
    let mut client_addr: std::net::SocketAddr;

    loop {
        // Receive data from the client.
        let (len, addr) = socket.recv_from(&mut buf).await?;
        let data = &buf[..len];
        client_addr = addr;

        if data == b"END" {
            println!("End of transmission received.");
            break;
        }

        // Extract the sequence number.
        if len < 4 {
            println!("Received invalid packet.");
            continue;
        }
        let seq_num = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let payload = &data[4..];

        // Check if the sequence number is as expected.
        if seq_num == expected_seq_num {
            println!("Received packet with sequence number: {}", seq_num);
            received_image_data.extend_from_slice(payload);
            expected_seq_num += 1;
        } else {
            println!(
                "Out of order packet received. Expected: {}, got: {}. Discarding.",
                expected_seq_num, seq_num
            );
        }

        // Send ACK for the last received sequence number.
        socket.send_to(&seq_num.to_be_bytes(), addr).await?;
    }

    // // encrypt_image(&mut received_image_data);
    println!("Received image data. Encrypting...");
    let encrypted_image_data = encrypt_image(received_image_data.clone(), "server.webp");
    println!("Image data encrypted.");

    fs::remove_file("hidden_message.png").unwrap();

    // Send the image data back to the client in chunks.
    const CHUNK_SIZE: usize = 1020;
    let mut seq_num: u32 = 0;
    for chunk in encrypted_image_data.chunks(CHUNK_SIZE) {
        let mut packet = vec![0; 4 + chunk.len()];
        packet[..4].copy_from_slice(&seq_num.to_be_bytes());
        packet[4..].copy_from_slice(chunk);

        // Send the packet with a sequence number.
        loop {
            socket.send_to(&packet, client_addr).await?;
            println!("Sent packet with sequence number: {}", seq_num);

            // Wait for an ACK from the client.
            let mut ack_buf = [0; 4];
            match timeout(Duration::from_secs(1), socket.recv_from(&mut ack_buf)).await {
                Ok(Ok((_, _))) => {
                    let ack_seq_num = u32::from_be_bytes(ack_buf);
                    if ack_seq_num == seq_num {
                        println!("Received ACK for sequence number: {}", ack_seq_num);
                        break; // Move to the next packet.
                    }
                }
                _ => {
                    println!("Timeout or error, resending sequence number: {}", seq_num);
                }
            }
        }

        seq_num += 1;
    }

    // Signal the end of transmission.
    socket.send_to(b"END", client_addr).await?;
    println!("End of transmission signal sent to client.");

    Ok(())

}