use std::fs::File;

use::steganography;
use steganography::encoder::Encoder;
use steganography::decoder::Decoder;
use::steganography::util::*;

pub fn encrypt_image(image_bytes: Vec<u8>, hidden_image_path: &str) -> Vec<u8> {

    let payload = image_bytes.clone();
    //Load the image where we want to embed our secret message
    let destination_image = file_as_dynamic_image(hidden_image_path.to_string());
    //Create an encoder
    let enc = Encoder::new(&payload, destination_image);
    //Encode our message into the alpha channel of the image
    let result = enc.encode_alpha();
    //Save the new image
    save_image_buffer(result, "hidden_message.png".to_string());

    let result_bytes = file_to_bytes(File::open("hidden_message.png").unwrap());

    result_bytes
    
}


pub fn decrypt_image() {



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

fn main() {

    let image_bytes = file_to_bytes(File::open("client.jpeg").unwrap());

    // Encrypting
    let _en = encrypt_image(image_bytes, "ex1.jpg");

    // Decrypting
   decrypt_image();
    println!("YESSSSSS");
}