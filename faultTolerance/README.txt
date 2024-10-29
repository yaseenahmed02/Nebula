- The enum represents the possible states of a server

- The type State vector holds the states of each server so each server is aware of the others through this shared states
- Mutex makes it synchronized in reading/modifying states
- If any server goes DOWN, it is shown in this shared state which other servers can see
-  Each server can check this to see which servers are UP or DOWN
- They don't communicate directly since they simply access the same shared data

fn update_status:
- down_count to track how many servers are down and make sure only 1 server is down at a time
- Randomly picks a server id to be down 
- Lock states to allow concurrent access
- Server is made down using thread:sleep (there’s also sleep in tokio library)

fn elect:
- Goes through state of each server
- Returns server ID of any server that is UP

fn handle_request:
- Dummy client requests for testing
- Uses elect function to find available server
- If server is UP and elected, ensures it’s up and returns it’s ID
- When server is elected, handles request (using sleep here to imitate the time taken to handle request) 
- Checking server status once more during processing of request in case server fails during request. 
      - If this happens, skips rest of processing for this request and continues to start loop iteration again 
        and re-elects another UP server to handle the request
