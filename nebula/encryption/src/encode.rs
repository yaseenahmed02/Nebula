extern crate steganography;
use steganography::encoder::*;
use steganography::util::*;

fn main() {
    let message = "This is a secret message".to_string();
    let payload = str_to_bytes(&message);
    
    let destination_image = file_as_dynamic_image("rust.png".to_string());
    let enc = Encoder::new(payload, destination_image);
    
    let result = enc.encode_alpha(); //alpha channel represents transparency info of each pixel
    save_image_buffer(result, "encodedIMG.png".to_string());
}
