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
const ADDRESS: &str = "127.0.0.1:4221";
const OK_HEADER: &str = "HTTP/1.1 200 OK\r\n\r\n";
const CREATED_HEADER: &str = "HTTP/1.1 201 Created\r\n\r\n";
const NOT_FOUND_HEADER: &str = "HTTP/1.1 404 Not Found\r\n\r\n";
const METHOD_NOT_ALLOWED_HEADER: &str = "HTTP/1.1 405 Method Not Allowed\r\n\r\n";

fn main() -> Result<(), Box<dyn Error>> {
    let directory = handle_args().unwrap_or_else(|err| {
        eprintln!(
            "Error: {}. Using default directory: {}",
            err, DEFAULT_DIRECTORY
        );
        DEFAULT_DIRECTORY.to_string()
    });

    let listener = TcpListener::bind(ADDRESS)?;

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
        Ok(args[2].to_owned())
    } else if args.len() > 1 {
        Err("Usage: program --directory <path>".into())
    } else {
        Ok(DEFAULT_DIRECTORY.to_string())
    }
}

fn handle_client(mut stream: TcpStream, directory: &str) -> Result<(), Box<dyn Error>> {
    let mut buf_reader = BufReader::new(&mut stream);

    let (method, path, headers) = parse_request(&mut buf_reader)?;
    let body = read_body(&mut buf_reader, &headers)?;

    let response = match method.as_str() {
        "POST" => handle_post(&path, &body, directory),
        "GET" => handle_get(&path, &headers, directory),
        _ => Ok(METHOD_NOT_ALLOWED_HEADER.to_string()),
    }?;

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    println!("Response sent successfully");
    Ok(())
}

fn parse_request<R: BufRead>(reader: &mut R) -> Result<(String, String, String), Box<dyn Error>> {
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();

    let mut headers = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line.trim().is_empty() {
            break;
        }
        headers.push_str(&line);
    }

    Ok((method, path, headers))
}

fn read_body<R: BufRead>(reader: &mut R, headers: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let content_length: usize = headers
        .lines()
        .find(|line| line.to_lowercase().starts_with("content-length:"))
        .map(|line| {
            line.split_whitespace()
                .nth(1)
                .unwrap_or("0")
                .parse()
                .unwrap_or(0)
        })
        .unwrap_or(0);

    let mut body = vec![0; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    Ok(body)
}

fn handle_post(path: &str, body: &[u8], directory: &str) -> Result<String, Box<dyn Error>> {
    if path.starts_with("/files/") {
        let filename = &path[7..];
        let filepath = Path::new(directory).join(filename);

        let mut file = File::create(filepath)?;
        file.write_all(body)?;

        Ok(CREATED_HEADER.to_string())
    } else {
        Ok(METHOD_NOT_ALLOWED_HEADER.to_string())
    }
}

fn handle_get(path: &str, headers: &str, directory: &str) -> Result<String, Box<dyn Error>> {
    if path.starts_with("/files/") {
        let filename = &path[7..];
        let filepath = Path::new(directory).join(filename);
        if filepath.exists() {
            serve_file(filepath, headers)
        } else {
            Ok(NOT_FOUND_HEADER.to_string())
        }
    } else if path == "/user-agent" {
        let user_agent = extract_user_agent(headers)?;
        serve_user_agent(&user_agent, headers)
    } else if path.starts_with("/echo/") {
        serve_echo(path, headers)
    } else if path == "/" {
        Ok(OK_HEADER.to_string())
    } else {
        Ok(NOT_FOUND_HEADER.to_string())
    }
}

fn extract_user_agent(headers: &str) -> Result<String, Box<dyn Error>> {
    for line in headers.lines() {
        if line.to_lowercase().starts_with("user-agent:") {
            let user_agent = line["User-Agent:".len()..].trim().to_string();
            return Ok(user_agent);
        }
    }
    Ok(String::new())
}

fn serve_file(filepath: PathBuf, headers: &str) -> Result<String, Box<dyn Error>> {
    let mut file = File::open(filepath)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;
    let content_length = contents.len();

    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n",
        content_length
    );

    if supports_gzip(headers) {
        response.push_str("Content-Encoding: gzip\r\n");
    }

    response.push_str("\r\n");
    response.push_str(&String::from_utf8_lossy(&contents));
    Ok(response)
}

fn serve_user_agent(user_agent: &str, headers: &str) -> Result<String, Box<dyn Error>> {
    let content_length = user_agent.len();
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n",
        content_length
    );

    if supports_gzip(headers) {
        response.push_str("Content-Encoding: gzip\r\n");
    }

    response.push_str("\r\n");
    response.push_str(user_agent);
    Ok(response)
}

fn serve_echo(path: &str, headers: &str) -> Result<String, Box<dyn Error>> {
    let echo_str = &path[6..];
    let content_length = echo_str.len();
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n",
        content_length
    );

    if supports_gzip(headers) {
        response.push_str("Content-Encoding: gzip\r\n");
    }

    response.push_str("\r\n");
    response.push_str(echo_str);
    Ok(response)
}

fn supports_gzip(headers: &str) -> bool {
    headers
        .lines()
        .find(|line| line.to_lowercase().starts_with("accept-encoding:"))
        .map(|line| {
            line["accept-encoding:".len()..]
                .split(',')
                .map(str::trim)
                .any(|encoding| encoding == "gzip")
        })
        .unwrap_or(false)
}
