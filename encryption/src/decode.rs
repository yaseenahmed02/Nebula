extern crate steganography;
use steganography::decoder::*;
use steganography::util::*;

pub fn decrypt_image(image_path: &str) -> Result<String, String> {
    // Load the encrypted image
    let encoded_image = file_as_image_buffer(image_path.to_string());

    // Create a decoder for the image
    let dec = Decoder::new(encoded_image);

    // Decode the message hidden in the alpha channel
    let out_buffer = dec.decode_alpha();

    // Filter out fully opaque pixels
    let clean_buffer: Vec<u8> = out_buffer.into_iter().filter(|b| *b != 0xff_u8).collect();

    // Convert the byte data back to a string
    let message = bytes_to_str(clean_buffer.as_slice());

    Ok(message.to_string())
}

// Add a main function to test decryption standalone
fn main() {
    let encrypted_image_path = "encrypted_image_received.png";

    match decrypt_image(encrypted_image_path) {
        Ok(message) => println!("Decrypted message: {:?}", message),
        Err(e) => eprintln!("Decryption failed: {}", e),
    }
}
