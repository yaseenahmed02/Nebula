use tonic::transport::Channel;
use message::messenger_client::MessengerClient; // Adjust based on your service
use message::MessageRequest; // Adjust based on your service

pub mod message {
    tonic::include_proto!("message");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = MessengerClient::connect("http://[::1]:50052").await?; // Change port to 50052

    let request = tonic::Request::new(MessageRequest {
        content: String::from("Hello, Tonic!"), // Example message
    });

    let response = client.send_message(request).await?; // Call the send_message method
    println!("Response: {:?}", response.into_inner().reply);

    Ok(())
}
