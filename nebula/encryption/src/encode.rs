extern crate steganography;
use steganography::encoder::*;
use steganography::util::*;

// Add this main function if you want to run encode.rs as a standalone binary
fn main() {
    let image_path = "rust.png";
    let message = "This is a secret message";
    let output_path = "encodedIMG.png";

    match encrypt_image(image_path, message, output_path) {
        Ok(_) => println!("Image successfully encrypted and saved to {}", output_path),
        Err(e) => eprintln!("Encryption failed: {}", e),
    }
}

pub fn encrypt_image(image_path: &str, message: &str, output_path: &str) -> Result<(), String> {
    let message_string = message.to_string(); // Store the string in a variable to avoid temporary lifetime issues
    let payload = str_to_bytes(&message_string);

    let destination_image = file_as_dynamic_image(image_path.to_string());

    let enc = Encoder::new(payload, destination_image);

    let result = enc.encode_alpha();

    save_image_buffer(result, output_path.to_string());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_image() {
        let result = encrypt_image("rust.png", "Test secret message", "encoded_test.png");
        assert!(result.is_ok());
    }
}
