use message::messenger_client::MessengerClient;
use message::ImageRequest;

pub mod message {
    tonic::include_proto!("message");
}

// Import decrypt_image from the encryption crate
use encryption::decode::decrypt_image;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the server at the specified address
    let mut client = MessengerClient::connect("http://[::1]:50052").await?;

    // The image path is relative to the "client" directory
    let image_path = "../encryption/rust.png"; // Path to the original image
    let image_data = std::fs::read(image_path)?; // Read the image as bytes

    // Create an ImageRequest with the binary image data
    let request = tonic::Request::new(ImageRequest {
        image_data, // Send the binary image data
    });

    // Send the image request to the server and wait for the response
    let response = client.send_image(request).await?.into_inner();

    // Save the encrypted image returned by the server as "encrypted_image_received.png"
    let encrypted_image_path = "encrypted_image_received.png";
    std::fs::write(encrypted_image_path, response.encrypted_image)?;

    // Print a success message
    println!("Encrypted image saved as '{}'", encrypted_image_path);

    // Decrypt the image using the decrypt_image function from the encryption crate
    match decrypt_image(encrypted_image_path) {
        Ok(message) => println!("Decrypted message: {:?}", message),
        Err(e) => eprintln!("Decryption failed: {}", e),
    }

    Ok(())
}
