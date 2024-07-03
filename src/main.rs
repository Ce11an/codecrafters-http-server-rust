use std::{
    error::Error,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
};

fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:4221")?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("Accepted new connection");

                if let Err(e) = handle_client(stream) {
                    eprintln!("Error handling client: {}", e);
                }
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }

    Ok(())
}

fn handle_client(mut stream: TcpStream) -> Result<(), Box<dyn Error>> {
    let mut buf_reader = BufReader::new(&mut stream);

    let mut request_line = String::new();
    buf_reader.read_line(&mut request_line)?;

    let mut user_agent = String::new();

    for line in buf_reader.by_ref().lines() {
        let line = line?;
        if line.is_empty() {
            break;
        }
        if line.to_lowercase().starts_with("user-agent:") {
            user_agent = line["User-Agent:".len()..].trim().to_string();
        }
    }

    let path = request_line.split_whitespace().nth(1).unwrap_or("");

    let response = generate_response(path, &user_agent);
    stream.write_all(response.as_bytes())?;

    println!("Response sent successfully");
    Ok(())
}

fn generate_response(path: &str, user_agent: &str) -> String {
    if path == "/user-agent" {
        let content_length = user_agent.len();
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
            content_length, user_agent
        )
    } else if path.starts_with("/echo/") {
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
    }
}
