use std::fs::File;

use::steganography;
use steganography::encoder::Encoder;
use steganography::decoder::Decoder;
use::steganography::util::*;

pub fn encrypt_image(image_bytes: Vec<u8>, hidden_image_path: &str) -> Vec<u8> {
    println!("Starting encryption...");
    println!("Using hidden image path: {}", hidden_image_path);

    let payload = image_bytes.clone();
    
    // Load the image where we want to embed our secret message
    println!("Attempting to load carrier image...");
    let destination_image = file_as_dynamic_image(hidden_image_path.to_string());
    println!("Carrier image loaded successfully.");

    // Create an encoder
    let enc = Encoder::new(&payload, destination_image);
    println!("Encoder created successfully.");

    // Encode our message into the alpha channel of the image
    let result = enc.encode_alpha();
    println!("Message encoded into alpha channel.");

    // Save the new image
    println!("Saving encrypted image as hidden_message.png...");
    save_image_buffer(result, "hidden_message.png".to_string());
    println!("Encrypted image saved as hidden_message.png.");

    // Read the saved encrypted image
    println!("Reading back encrypted image from hidden_message.png...");
    let result_bytes = file_to_bytes(File::open("hidden_message.png").unwrap());
    println!("Encrypted image read successfully.");

    result_bytes
}

pub fn decrypt_image() {
    println!("Starting decryption...");
    println!("Attempting to load hidden_message.png for decoding...");

    let encoded_image = file_as_image_buffer("hidden_message.png".to_string());
    println!("Image loaded for decoding.");

    // Create a decoder
    let dec = Decoder::new(encoded_image);
    println!("Decoder created successfully.");

    // Decode the image by reading the alpha channel
    let out_buffer = dec.decode_alpha();
    println!("Decoded alpha channel data.");

    // Save the decoded message
    println!("Saving decoded image as decoded_image.png...");
    bytes_to_file(&out_buffer, &File::create("decoded_image.png").unwrap());
    println!("Decoded image saved as decoded_image.png.");
}

fn main() {
    println!("Starting main function...");

    println!("Attempting to load client image from /home/yaseen/Nebula/client/client.jpg...");
    let image_bytes = file_to_bytes(File::open("/home/yaseen/Nebula/client/client.jpg").unwrap());
    println!("Client image loaded successfully.");

    // Encrypting
    println!("Calling encrypt_image function...");
    let _en = encrypt_image(image_bytes, "/home/yaseen/Nebula/encrypt/encryption_img.jpg");

    // Decrypting
    println!("Calling decrypt_image function...");
    decrypt_image();
    
    println!("Process completed successfully.");
}
