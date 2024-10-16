use tonic::{transport::Server, Request, Response, Status};
use message::messenger_server::{Messenger, MessengerServer};
use message::{MessageRequest, MessageResponse};

pub mod message {
    tonic::include_proto!("message");
}

#[derive(Debug, Default)]
pub struct MyMessenger {}

#[tonic::async_trait]
impl Messenger for MyMessenger {
    async fn send_message(
        &self,
        request: Request<MessageRequest>,
    ) -> Result<Response<MessageResponse>, Status> {
        let message = request.into_inner().content;
        println!("Received message: {}", message);

        let reply = message.to_uppercase();
        let response = MessageResponse { reply };

        Ok(Response::new(response))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50052".parse()?; // Change port to 50052
    let messenger = MyMessenger::default();

    println!("Server listening on {}", addr);

    Server::builder()
        .add_service(MessengerServer::new(messenger))
        .serve(addr)
        .await?;

    Ok(())
}
