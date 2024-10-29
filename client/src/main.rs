use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};
use tokio::fs;
use std::{error::Error, fs::File};
use std::path::Path;
use steganography::util::{bytes_to_file,file_as_image_buffer};
use steganography::decoder::Decoder;


fn decrypt_image() {



    let encoded_image = file_as_image_buffer("hidden_message.png".to_string());
    //Create a decoder
    let dec = Decoder::new(encoded_image);
    //Decode the image by reading the alpha channel
    let out_buffer = dec.decode_alpha();
    //If there is no alpha, it's set to 255 by default so we filter those out
    // let clean_buffer: Vec<u8> = out_buffer.into_iter().filter(|b| *b != 0xff_u8).collect();
    //Convert those bytes into a string we can read
    // let message = bytes_to_str(clean_buffer.as_slice());
    bytes_to_file(&out_buffer, &File::create("decoded_image.png").unwrap());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Bind to an ephemeral local address.
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    let server_addr = "127.0.0.1:8080";
    let image_path = Path::new("client.jpeg");
    let image_data = fs::read(image_path).await?;
    println!("Image size: {} bytes", image_data.len());

    const CHUNK_SIZE: usize = 1020; // 1024 bytes - 4 bytes for the sequence number.
    let mut seq_num: u32 = 0;

    for chunk in image_data.chunks(CHUNK_SIZE) {
        let mut packet = vec![0; 4 + chunk.len()];
        packet[..4].copy_from_slice(&seq_num.to_be_bytes());
        packet[4..].copy_from_slice(chunk);

        // Send the packet with a sequence number.
        loop {
            socket.send_to(&packet, server_addr).await?;
            println!("Sent packet with sequence number: {}", seq_num);

            // Wait for an ACK from the server.
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
    socket.send_to(b"END", server_addr).await?;
    println!("End of transmission signal sent.");

    // Receive the image back in chunks.
    let mut received_image_data = Vec::new();
    let mut buf = [0; 1024];

    loop {
        let (len, _) = socket.recv_from(&mut buf).await?;
        if &buf[..len] == b"END" {
            break;
        }

        // Extract the sequence number from the received packet.
        if len < 4 {
            continue; // Invalid packet.
        }
        let seq_num = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let payload = &buf[4..len];
        println!("Received packet with sequence number: {}", seq_num);

        received_image_data.extend_from_slice(payload);

        // Send ACK for the received packet.
        socket.send_to(&seq_num.to_be_bytes(), server_addr).await?;
    }

    // Save the received image.
    bytes_to_file(&received_image_data, &File::create("hidden_message.png")?);
    println!("Received image saved.");

    // // Decode the image data.
    // decode_received_image().await?;
    // println!("Decoded image saved.");

    decrypt_image();

    Ok(())
}
