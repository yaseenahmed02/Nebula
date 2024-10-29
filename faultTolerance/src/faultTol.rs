use rand::Rng;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const NUM_SERVERS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Server {
    UP,
    DOWN,
}

type State = Arc<Mutex<Vec<Server>>>;

fn update_status(id_server: usize, states: State) {
    let mut rng = rand::thread_rng();
    loop {
        let fails = rng.gen_bool(0.5);
        let mut states_lock = states.lock().unwrap();

        let mut down_count = 0;
        for &st in states_lock.iter() {
            if st == Server::DOWN {
                down_count += 1;
            }
        }
        
        if fails && down_count == 0 {
            println!("Server {}: DOWN", id_server);
            states_lock[id_server] = Server::DOWN;
        } else {
            println!("Server {}: UP", id_server);
            states_lock[id_server] = Server::UP;
        }

        drop(states_lock);
        thread::sleep(Duration::from_secs(3));
    }
}


fn elect(states: &Vec<Server>) -> Option<usize> {
    for (id, &state) in states.iter().enumerate() {
        if state == Server::UP {
            return Some(id);
        }
    }
    None
}

fn handle_request(states: State) {
    loop {
        let elected = {
            let mut states_lock = states.lock().unwrap();
            let server_id = elect(&states_lock);

            if let Some(server_id) = server_id {
                states_lock[server_id] = Server::UP;
                Some(server_id)
            } else {
                None
            }
        };

        match elected {
            Some(server_id) => {
                println!("Request HANDLED BY Server {}", server_id);
                thread::sleep(Duration::from_secs(2));

                let server_status_changed = {
                    let states_lock = states.lock().unwrap();
                    states_lock[server_id] != Server::UP
                };

                if server_status_changed {
                    println!("Server {} status CHANGED during request. Assigning request to other server.", server_id);
                    continue; 
                }

                println!("Server {} FINISHED REQUEST", server_id);
            }
            None => println!("No Available Server"),
        }

        thread::sleep(Duration::from_secs(5));
    }
}

fn main() {
    let server_states: State = Arc::new(Mutex::new(vec![Server::UP; NUM_SERVERS]));

    for i in 0..NUM_SERVERS {
        let states_clone = Arc::clone(&server_states);
        thread::spawn(move || update_status(i, states_clone));
    }

    {
        let states_lock = server_states.lock().unwrap();
        println!("Initial Status:");
        for (i, &st) in states_lock.iter().enumerate() {
            println!("Server {}: {:?}", i, st);
        }
    }

    let handle_requests = {
        let states_clone = Arc::clone(&server_states);
        thread::spawn(move || handle_request(states_clone))
    };

    handle_requests.join().unwrap();
}
