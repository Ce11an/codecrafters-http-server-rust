use anyhow::{Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::{
    env,
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
const METHOD_NOT_ALLOWED_HEADER: &str = "HTTP/1.1 405 Method Not Allowed\r\n";

#[derive(Debug)]
struct Response {
    status_line: &'static str,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    fn build(&self) -> Vec<u8> {
        let mut response = Vec::new();
        response.extend_from_slice(self.status_line.as_bytes());
        for (key, value) in &self.headers {
            response.extend_from_slice(format!("{}: {}\r\n", key, value).as_bytes());
        }
        response.extend_from_slice(b"\r\n");
        response.extend_from_slice(&self.body);
        response
    }
}

fn main() -> Result<()> {
    let directory = match handle_args() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!(
                "Error: {}. Using default directory: {}",
                err, DEFAULT_DIRECTORY
            );
            DEFAULT_DIRECTORY.to_string()
        }
    };

    let listener = TcpListener::bind(ADDRESS)?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let directory = directory.clone();
                thread::spawn(move || {
                    if let Err(e) = handle_client(stream, &directory) {
                        eprintln!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }

    Ok(())
}

fn handle_args() -> Result<String> {
    let args: Vec<String> = env::args().collect();
    if args.len() == 3 && args[1] == "--directory" {
        Ok(args[2].clone())
    } else if args.len() > 1 {
        Err(anyhow::anyhow!("Usage: program --directory <path>"))
    } else {
        Ok(DEFAULT_DIRECTORY.to_string())
    }
}

fn handle_client(mut stream: TcpStream, directory: &str) -> Result<()> {
    let mut buf_reader = BufReader::new(&mut stream);

    let (method, path, headers) = parse_request(&mut buf_reader)?;
    let body = read_body(&mut buf_reader, &headers)?;

    let response = match method.as_str() {
        "POST" => handle_post(&path, &body, directory),
        "GET" => handle_get(&path, &headers, directory),
        _ => Ok(Response {
            status_line: METHOD_NOT_ALLOWED_HEADER,
            headers: vec![],
            body: vec![],
        }),
    }?;

    stream.write_all(&response.build())?;
    stream.flush()?;

    println!("Response sent successfully");
    Ok(())
}

fn parse_request<R: BufRead>(reader: &mut R) -> Result<(String, String, String)> {
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .context("Failed to read request line")?;
    let request_line = request_line.trim();

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();

    let mut headers = String::new();
    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .context("Failed to read header line")?;
        if line.trim().is_empty() {
            break;
        }
        headers.push_str(&line);
    }

    Ok((method, path, headers))
}

fn read_body<R: BufRead>(reader: &mut R, headers: &str) -> Result<Vec<u8>> {
    let content_length: usize = headers
        .lines()
        .find(|line| line.to_lowercase().starts_with("content-length:"))
        .and_then(|line| line.split_whitespace().nth(1)?.parse().ok())
        .unwrap_or(0);

    let mut body = vec![0; content_length];
    if content_length > 0 {
        reader
            .read_exact(&mut body)
            .context("Failed to read body")?;
    }

    Ok(body)
}

fn handle_post(path: &str, body: &[u8], directory: &str) -> Result<Response> {
    if path.starts_with("/files/") {
        let filename = &path[7..];
        let filepath = Path::new(directory).join(filename);

        File::create(filepath)?
            .write_all(body)
            .context("Failed to write file")?;
        Ok(Response {
            status_line: CREATED_HEADER,
            headers: vec![],
            body: vec![],
        })
    } else {
        Ok(Response {
            status_line: METHOD_NOT_ALLOWED_HEADER,
            headers: vec![],
            body: vec![],
        })
    }
}

fn handle_get(path: &str, headers: &str, directory: &str) -> Result<Response> {
    if path.starts_with("/files/") {
        let filename = &path[7..];
        let filepath = Path::new(directory).join(filename);
        if filepath.exists() {
            serve_file(filepath, headers)
        } else {
            Ok(Response {
                status_line: NOT_FOUND_HEADER,
                headers: vec![],
                body: vec![],
            })
        }
    } else if path == "/user-agent" {
        let user_agent = extract_user_agent(headers)?;
        serve_user_agent(&user_agent, headers)
    } else if path.starts_with("/echo/") {
        serve_echo(path, headers)
    } else if path == "/" {
        Ok(Response {
            status_line: OK_HEADER,
            headers: vec![],
            body: vec![],
        })
    } else {
        Ok(Response {
            status_line: NOT_FOUND_HEADER,
            headers: vec![],
            body: vec![],
        })
    }
}

fn extract_user_agent(headers: &str) -> Result<String> {
    for line in headers.lines() {
        if line.to_lowercase().starts_with("user-agent:") {
            return Ok(line["User-Agent:".len()..].trim().to_string());
        }
    }
    Ok(String::new())
}

fn serve_file(filepath: PathBuf, headers: &str) -> Result<Response> {
    let mut file = File::open(filepath)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)
        .context("Failed to read file")?;

    let content_length = contents.len();
    let supports_gzip = supports_gzip(headers);

    let mut response = Response {
        status_line: "HTTP/1.1 200 OK\r\n",
        headers: vec![(
            "Content-Type".to_string(),
            "application/octet-stream".to_string(),
        )],
        body: vec![],
    };

    if supports_gzip {
        let compressed_contents = compress_gzip(&contents)?;
        response
            .headers
            .push(("Content-Encoding".to_string(), "gzip".to_string()));
        response.body.extend_from_slice(&compressed_contents);
        response.headers.push((
            "Content-Length".to_string(),
            compressed_contents.len().to_string(),
        ));
    } else {
        response.body.extend_from_slice(&contents);
        response
            .headers
            .push(("Content-Length".to_string(), content_length.to_string()));
    }

    Ok(response)
}

fn serve_user_agent(user_agent: &str, headers: &str) -> Result<Response> {
    let supports_gzip = supports_gzip(headers);

    let response_body = user_agent.as_bytes();
    let content_length = response_body.len();

    let mut response = Response {
        status_line: "HTTP/1.1 200 OK\r\n",
        headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
        body: vec![],
    };

    if supports_gzip {
        let compressed_contents = compress_gzip(response_body)?;
        response
            .headers
            .push(("Content-Encoding".to_string(), "gzip".to_string()));
        response.body.extend_from_slice(&compressed_contents);
        response.headers.push((
            "Content-Length".to_string(),
            compressed_contents.len().to_string(),
        ));
    } else {
        response.body.extend_from_slice(response_body);
        response
            .headers
            .push(("Content-Length".to_string(), content_length.to_string()));
    }

    Ok(response)
}

fn serve_echo(path: &str, headers: &str) -> Result<Response> {
    let echo_str = &path[6..];
    let supports_gzip = supports_gzip(headers);

    let response_body = echo_str.as_bytes();
    let content_length = response_body.len();

    let mut response = Response {
        status_line: "HTTP/1.1 200 OK\r\n",
        headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
        body: vec![],
    };

    if supports_gzip {
        let compressed_contents = compress_gzip(response_body)?;
        response
            .headers
            .push(("Content-Encoding".to_string(), "gzip".to_string()));
        response.body.extend_from_slice(&compressed_contents);
        response.headers.push((
            "Content-Length".to_string(),
            compressed_contents.len().to_string(),
        ));
    } else {
        response.body.extend_from_slice(response_body);
        response
            .headers
            .push(("Content-Length".to_string(), content_length.to_string()));
    }

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

fn compress_gzip(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    encoder.finish().map_err(Into::into)
}
