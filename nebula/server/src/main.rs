use tonic::{transport::Server, Request, Response, Status};
use message::messenger_server::{Messenger, MessengerServer};
use message::{ImageRequest, ImageResponse};

pub mod message {
    tonic::include_proto!("message");
}

// Import the encrypt_image function from the encryption crate
use encryption::encode::encrypt_image;

#[derive(Debug, Default)]
pub struct MyMessenger {}

#[tonic::async_trait]
impl Messenger for MyMessenger {
    async fn send_image(
        &self,
        request: Request<ImageRequest>, // Receive an image in the request
    ) -> Result<Response<ImageResponse>, Status> {
        let image_data = request.into_inner().image_data; // Get image data from the request
        println!("Received image of size: {}", image_data.len());

        // Save the received image as a temporary file
        let input_image_path = "received_image.png";
        if std::fs::write(input_image_path, image_data).is_err() {
            return Err(Status::internal("Failed to save received image"));
        }

        // Call the encrypt_image function to encrypt the image
        let encrypted_image_path = "encrypted_image.png";
        match encrypt_image(input_image_path, "This is a secret message", encrypted_image_path) {
            Ok(_) => {
                // Read the encrypted image from the file
                let encrypted_image_data = std::fs::read(encrypted_image_path).map_err(|e| {
                    Status::internal(format!("Failed to read encrypted image: {:?}", e))
                })?;

                // Send the encrypted image back in the response
                let response = ImageResponse {
                    encrypted_image: encrypted_image_data,
                };

                Ok(Response::new(response))
            }
            Err(e) => {
                println!("Encryption failed: {}", e);
                Err(Status::internal(format!("Encryption failed: {}", e)))
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50052".parse()?; // Server address
    let messenger = MyMessenger::default();

    println!("Server listening on {}", addr);

    Server::builder()
        .add_service(MessengerServer::new(messenger)) // Add the Messenger service
        .serve(addr)
        .await?;

    Ok(())
}
