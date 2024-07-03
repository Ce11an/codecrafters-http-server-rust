use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").expect("Failed to bind!");

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                println!("Accepted new connection");

                let mut buf_reader = BufReader::new(&mut stream);

                let mut request_line = String::new();
                match buf_reader.read_line(&mut request_line) {
                    Ok(_) => {
                        let path = request_line.split_whitespace().nth(1).unwrap_or("");

                        let response = if path == "/" {
                            "HTTP/1.1 200 OK\r\n\r\n"
                        } else {
                            "HTTP/1.1 404 Not Found\r\n\r\n"
                        };

                        match stream.write_all(response.as_bytes()) {
                            Ok(_) => println!("Response sent successfully"),
                            Err(e) => println!("Failed to write response: {}", e),
                        }
                    }
                    Err(e) => println!("Failed to read request line: {}", e),
                }
            }
            Err(e) => println!("Error: {}", e),
        }
    }
}
