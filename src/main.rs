use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time::error::Elapsed;
use std::env;
use std::error::Error;
use std::sync::Arc;
use tokio::time::{self, Duration, Instant};
use sysinfo::System;
use tokio::signal;

// ServerNode and State structs
#[derive(Clone, Debug)]
struct ServerNode {
	id: u8,
	address: String,
	cpu_utilization: f32,
	last_updated: Instant,
	online: bool,
	//server_in_use: bool

}

struct State {
	leader: Mutex<Option<ServerNode>>,
	known_nodes: Mutex<Vec<ServerNode>>,
	num_peers: usize,
	timeout_duration: Duration,
}

struct ClientRequest {
    client_id: String,
    request_number: u32,
    handled: bool,
}


// State implementation with leader election
impl State {
	async fn set_leader(&self, node: ServerNode) {
    	let mut leader = self.leader.lock().await;
    	*leader = Some(node.clone());
    	println!("Leader updated to node with ID {} and CPU: {:.2}%", node.id, node.cpu_utilization);

    	// Announce new leader to all nodes and clients
    	self.announce_leader().await;
	}

	async fn get_leader(&self) -> Option<ServerNode> {
    	let leader = self.leader.lock().await;
    	leader.clone()
	}

	async fn update_node_info(&self, node: ServerNode) {
    	let mut nodes = self.known_nodes.lock().await;
    	if let Some(existing_node) = nodes.iter_mut().find(|n| n.id == node.id) {
        	*existing_node = node; // Update existing node
    	} else {
        	nodes.push(node); // Add new node
    	}
	}

	async fn mark_offline_nodes(&self) {
    	let mut nodes = self.known_nodes.lock().await;
    	for node in nodes.iter_mut() {
        	if node.last_updated.elapsed() > self.timeout_duration {
            	node.online = false;
        	}
    	}
	}

	async fn elect_leader_based_on_cpu(&self) {
    	self.mark_offline_nodes().await;
    	let nodes = self.known_nodes.lock().await;
    	let online_nodes: Vec<&ServerNode> = nodes.iter().filter(|n| n.online).collect();

    	if online_nodes.len() >= self.num_peers.min(2) {
        	if let Some(least_cpu_node) = online_nodes.iter().min_by(|a, b| a.cpu_utilization.partial_cmp(&b.cpu_utilization).unwrap()) {
            	self.set_leader((*least_cpu_node).clone()).await;
        	}
    	} else {
        	println!("Not enough nodes online to perform leader election.");
    	}
	}

	async fn announce_leader(&self) {
    	if let Some(leader) = self.get_leader().await {
        	let announcement_msg = format!("leader:{}:{}", leader.id, leader.address);
        	let known_nodes = self.known_nodes.lock().await;
        	for node in known_nodes.iter() {
            	let socket = UdpSocket::bind("10.7.16.146:8082").await.unwrap();
            	socket.send_to(announcement_msg.as_bytes(), &node.address).await.unwrap();
        	}
    	}
	}
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
	let state = Arc::new(State {
    	leader: Mutex::new(None),
    	known_nodes: Mutex::new(Vec::new()),
    	num_peers: 3,
    	timeout_duration: Duration::from_secs(10),
	});

	let server_id = 2;
	let server_address = "10.7.16.146:8080";
	let peers = vec!["10.7.16.133:8080", "10.7.16.130:8080"];

	// Server-to-server communication task for leader election
	let state_clone = Arc::clone(&state);
	let election_task = tokio::spawn(async move {
    	let mut sys = System::new_all();
    	loop {
        	sys.refresh_cpu_usage();
        	let cpu_utilizations: Vec<f32> = sys.cpus().iter().map(|cpu| cpu.cpu_usage()).collect();
        	let cpu_utilization: f32 = cpu_utilizations.iter().copied().sum::<f32>() / cpu_utilizations.len() as f32;

        	for &peer in &peers {
            	let msg = format!("cpu:{}:{}", server_id, cpu_utilization);
            	let socket = UdpSocket::bind("10.7.16.146:8081").await.unwrap();
            	socket.send_to(msg.as_bytes(), peer).await.unwrap();
        	}

        	let node = ServerNode {
            	id: server_id,
            	address: server_address.to_string(),
            	cpu_utilization,
            	last_updated: Instant::now(),
            	online: true,
				//server_in_use: false,
        	};
        	state_clone.update_node_info(node).await;
        	state_clone.elect_leader_based_on_cpu().await;
        	time::sleep(Duration::from_secs(5)).await;
    	}
	});

	// Listener task to receive node updates and client leader requests
	let state_listener = Arc::clone(&state);
	let listener_task = tokio::spawn(async move {
    	let socket = UdpSocket::bind(server_address).await.unwrap();
    	let mut buf = [0; 1024];
    	loop {
        	let (len, addr) = socket.recv_from(&mut buf).await.unwrap();
        	let message = String::from_utf8_lossy(&buf[..len]);
        	if message.starts_with("cpu:") {
            	let parts: Vec<&str> = message.split(':').collect();
            	if parts.len() == 3 {
                	let node_id: u8 = parts[1].parse().unwrap();
                	let cpu_utilization: f32 = parts[2].parse().unwrap();
                	let node = ServerNode {
                    	id: node_id,
                    	address: addr.to_string(),
                    	cpu_utilization,
                    	last_updated: Instant::now(),
                    	online: true,
						//server_in_use: false,
                	};
                	state_listener.update_node_info(node).await;
                	state_listener.elect_leader_based_on_cpu().await;
            	}
        	} else if message == "leader?" {
            	// Respond to client with the current leader's address
				println!("hello");
            	if let Some(leader) = state_listener.get_leader().await {
                	let leader_msg = format!("current leader:{}:{}", leader.id, leader.address);
                	socket.send_to(leader_msg.as_bytes(), addr).await.unwrap();
            	}
        	}
    	}
	});

	// Client handling task to respond to multiple concurrent requests if this server is the leader
    let state_for_clients = Arc::clone(&state);
    let client_socket = Arc::new(UdpSocket::bind("10.7.16.146:8082").await.unwrap());
    let mut buf = [0; 1024];

	//Caching client requets
	//let request_cache: Arc<Mutex<Vec<ClientRequest>>> = Arc::new(Mutex::new(Vec::new()));

    let client_task = tokio::spawn(async move {
        loop {
            let (len, client_addr) = client_socket.recv_from(&mut buf).await.expect("ERROR");
            let message = String::from_utf8_lossy(&buf[..len]).to_string(); // Make sure `message` is owned
            
			// Extract client_id and request_number from the message
			// let parts: Vec<&str> = message.split(':').collect();
			// if parts.len() < 3 {
			// 	continue; // Skip malformed messages
			// }
			// let client_id = parts[1].to_string();
			// let request_number: u32 = match parts[2].parse() {
			// 	Ok(num) => num,
			// 	Err(_) => continue, // Skip if parsing fails
			// };

			//  // Check the cache first
			//  {
            //     let cache = request_cache.lock().await;
            //     if cache.iter().any(|req| req.client_id == client_id && req.request_number == request_number && req.handled) {
            //         // If already handled, respond to the client that it was already processed
            //         let response = format!("Request {} from client {} has already been handled.", request_number, client_id);
            //         client_socket.send_to(response.as_bytes(), client_addr).await.unwrap();
            //         continue;
            //     }
            // }

            // Spawn a new task for each client request
            let state = Arc::clone(&state_for_clients);
            let client_socket = Arc::clone(&client_socket);
            //let request_cache = Arc::clone(&request_cache);


            tokio::spawn(async move {
                if message.starts_with("request:") {
                    // Check if current server is the leader
                    if let Some(leader_node) = state.get_leader().await {
                        if leader_node.id == server_id {
                            let response = format!("response from leader {}: processed '{}'", server_id, message);
                            client_socket.send_to(response.as_bytes(), client_addr).await.unwrap();
                        }
                    }
                }
            });
        }
    });


	//Add this after the server is done handling the encryption for the client
	// // Update the cache to mark the request as handled
	// let mut cache = request_cache.lock().unwrap();
	// cache.push(ClientRequest {
	// 	client_id,
	// 	request_number,
	// 	handled: true,
	// });

	signal::ctrl_c().await.expect("stop error not working");
	println!("Server shutting down");

	// election_task.await.unwrap();
	// listener_task.await.unwrap();
	// client_task.await.unwrap();
	Ok(())
}





























































































// #[derive(Clone, Debug)]
// struct ServerNode {
//     id: u8,
//     address: String,
//     cpu_utilization: f32,
//     last_updated: Instant,
//     online: bool,
// }

// struct State {
//     leader: Mutex<Option<ServerNode>>,
//     known_nodes: Mutex<Vec<ServerNode>>,
//     num_peers: usize,
//     timeout_duration: Duration,
// }

// impl State {
//     async fn set_leader(&self, node: ServerNode) {
//         let mut leader = self.leader.lock().await;
//         *leader = Some(node.clone());
//         println!("Leader updated to node with ID {} and CPU: {:.2}%", node.id, node.cpu_utilization);
//     }

//     async fn get_leader(&self) -> Option<ServerNode> {
//         let leader = self.leader.lock().await;
//         leader.clone()
//     }

//     async fn update_node_info(&self, node: ServerNode) {
//         let mut nodes = self.known_nodes.lock().await;
//         if let Some(existing_node) = nodes.iter_mut().find(|n| n.id == node.id) {
//             *existing_node = node;  // Update existing node
//         } else {
//             nodes.push(node);  // Add new node
//         }
//     }

//     async fn mark_offline_nodes(&self) {
//         let mut nodes = self.known_nodes.lock().await;
//         for node in nodes.iter_mut() {
//             if node.last_updated.elapsed() > self.timeout_duration {
//                 node.online = false;
//             }
//         }
//     }

//     async fn elect_leader_based_on_cpu(&self) {
//         self.mark_offline_nodes().await;

//         let nodes = self.known_nodes.lock().await;
//         let online_nodes: Vec<&ServerNode> = nodes.iter().filter(|n| n.online).collect();

//         if online_nodes.len() >= self.num_peers.min(2) {
//             if let Some(least_cpu_node) = online_nodes.iter().min_by(|a, b| a.cpu_utilization.partial_cmp(&b.cpu_utilization).unwrap()) {
//                 self.set_leader((*least_cpu_node).clone()).await;
//             }
//         } else {
//             println!("Not enough nodes online to perform leader election.");
//         }
//     }
// }

// #[tokio::main]
// async fn main() -> std::io::Result<()> {
//     let state = Arc::new(State {
//         leader: Mutex::new(None),
//         known_nodes: Mutex::new(Vec::new()),
//         num_peers: 3,
//         timeout_duration: Duration::from_secs(10),
//     });

//     let server_id = 2;
//     let server_address = "10.7.16.146:8080";
//     let peers = vec!["10.7.16.133:8080", "10.7.16.130:8080"];



//     let state_clone = Arc::clone(&state);
//     let t1 = tokio::spawn(async move {
//         let mut sys = System::new_all();
//         //println!("election");

//         loop {
//             sys.refresh_cpu_usage();
//             //let cpu_utilization = sys.global_processor_info().cpu_usage();

//             let cpu_utilizations: Vec<f32> = sys.cpus().iter().map(|cpu| cpu.cpu_usage()).collect(); //Collecting CPU utilization from all of them
//             let cpu_utilization: f32 = cpu_utilizations.iter().copied().sum::<f32>() / cpu_utilizations.len() as f32; //Getting the average of Utilization

//             for &peer in &peers {
//                 let msg = format!("cpu:{}:{}", server_id, cpu_utilization);
//                 let socket = UdpSocket::bind("10.7.16.146:8081").await.unwrap();
//                 socket.send_to(msg.as_bytes(), peer).await.unwrap();
//             }

//             let node = ServerNode {
//                 id: server_id,
//                 address: server_address.to_string(),
//                 cpu_utilization,
//                 last_updated: Instant::now(),
//                 online: true,
//             };
//             state_clone.update_node_info(node).await;

//             state_clone.elect_leader_based_on_cpu().await;

//             time::sleep(Duration::from_secs(5)).await;
//         }
//     });

//   let state_listener = Arc::clone(&state);
//   let t2=   tokio::spawn(async move {
//         let socket = UdpSocket::bind(server_address).await.unwrap();
//         let mut buf = [0; 1024];

//         //println!("listener");
//         loop {
//             let (len, _addr) = socket.recv_from(&mut buf).await.unwrap();
//             let message = String::from_utf8_lossy(&buf[..len]);

//             if message.starts_with("cpu:") {
//                 let parts: Vec<&str> = message.split(':').collect();
//                 if parts.len() == 3 {
//                     let node_id: u8 = parts[1].parse().unwrap();
//                     let cpu_utilization: f32 = parts[2].parse().unwrap();

//                     let node = ServerNode {
//                         id: node_id,
//                         address: _addr.to_string(),
//                         cpu_utilization,
//                         last_updated: Instant::now(),
//                         online: true,
//                     };
//                     state_listener.update_node_info(node).await;

//                     state_listener.elect_leader_based_on_cpu().await;
//                 }
//             }
//         }
//     });

//     //tokio::signal::ctrl_c().await.unwrap();
    
//     //    
//     //Keep the main function running indefinitely
//     // loop {
//     //     time::sleep(Duration::from_secs(60)).await;
//     // }

    

//     t1.await.unwrap();
//     t2.await.unwrap();
//     Ok(())

// }



















































































