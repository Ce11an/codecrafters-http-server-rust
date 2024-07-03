use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                println!("Accepted new connection");

                let mut buf_reader = BufReader::new(&mut stream);

                let mut request_line = String::new();
                match buf_reader.read_line(&mut request_line) {
                    Ok(_) => {
                        let path = request_line.split_whitespace().nth(1).unwrap_or("");

                        let response = if path.starts_with("/echo/") {
                            let echo_str = &path[6..];
                            let content_length = echo_str.len();
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                                content_length, echo_str
                            )
                        } else if path == "/" {
                            "HTTP/1.1 200 OK\r\n\r\n".to_string()
                        } else {
                            "HTTP/1.1 404 Not Found\r\n\r\n".to_string()
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
