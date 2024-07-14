use std::{
    env,
    error::Error,
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    thread,
};

const DEFAULT_DIRECTORY: &str = ".";

fn main() -> Result<(), Box<dyn Error>> {
    let directory = handle_args().unwrap_or_else(|err| {
        eprintln!(
            "Error: {}. Using default directory: {}",
            err, DEFAULT_DIRECTORY
        );
        DEFAULT_DIRECTORY.to_string()
    });

    let listener = TcpListener::bind("127.0.0.1:4221")?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let directory_clone = directory.clone();
                thread::spawn(move || {
                    println!("Accepted new connection");
                    if let Err(e) = handle_client(stream, &directory_clone) {
                        eprintln!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }

    Ok(())
}

fn handle_args() -> Result<String, Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() == 3 && args[1] == "--directory" {
        Ok(args[2].clone())
    } else if args.len() > 1 {
        Err("Usage: program --directory <path>".into())
    } else {
        Ok(DEFAULT_DIRECTORY.to_string())
    }
}

fn handle_client(mut stream: TcpStream, directory: &str) -> Result<(), Box<dyn Error>> {
    let mut buf_reader = BufReader::new(&mut stream);

    let mut request_line = String::new();
    buf_reader.read_line(&mut request_line)?;

    let user_agent = extract_user_agent(&mut buf_reader)?;

    let path = request_line.split_whitespace().nth(1).unwrap_or("");

    let response = generate_response(path, &user_agent, directory)?;
    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    println!("Response sent successfully");
    Ok(())
}

fn extract_user_agent<R: BufRead>(reader: &mut R) -> Result<String, Box<dyn Error>> {
    let mut user_agent = String::new();
    for line in reader.lines() {
        let line = line?;
        if line.is_empty() {
            break;
        }
        if line.to_lowercase().starts_with("user-agent:") {
            user_agent = line["User-Agent:".len()..].trim().to_string();
        }
    }
    Ok(user_agent)
}

fn generate_response(
    path: &str,
    user_agent: &str,
    directory: &str,
) -> Result<String, Box<dyn Error>> {
    if path.starts_with("/files/") {
        let filename = &path[7..];
        let filepath = Path::new(directory).join(filename);
        if filepath.exists() {
            serve_file(filepath)
        } else {
            Ok("HTTP/1.1 404 Not Found\r\n\r\n".to_string())
        }
    } else if path == "/user-agent" {
        serve_user_agent(user_agent)
    } else if path.starts_with("/echo/") {
        serve_echo(path)
    } else if path == "/" {
        Ok("HTTP/1.1 200 OK\r\n\r\n".to_string())
    } else {
        Ok("HTTP/1.1 404 Not Found\r\n\r\n".to_string())
    }
}

fn serve_file(filepath: PathBuf) -> Result<String, Box<dyn Error>> {
    let mut file = File::open(filepath)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;
    let content_length = contents.len();
    Ok(format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n{}",
        content_length,
        String::from_utf8_lossy(&contents)
    ))
}

fn serve_user_agent(user_agent: &str) -> Result<String, Box<dyn Error>> {
    let content_length = user_agent.len();
    Ok(format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
        content_length, user_agent
    ))
}

fn serve_echo(path: &str) -> Result<String, Box<dyn Error>> {
    let echo_str = &path[6..];
    let content_length = echo_str.len();
    Ok(format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
        content_length, echo_str
    ))
}
