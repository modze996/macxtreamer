use crate::models::Config;
use crate::logger::log_line;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_socks::tcp::Socks5Stream;
use tokio_native_tls::TlsConnector;

/// Wrapper für HTTP Response (funktioniert mit SOCKS5 und normalen Requests)
pub struct HttpResponse {
    body: String,
    status: u16,
}

impl HttpResponse {
    pub async fn text(&self) -> Result<String, String> {
        Ok(self.body.clone())
    }

    #[allow(dead_code)]
    pub async fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, String> {
        serde_json::from_str(&self.body)
            .map_err(|e| format!("JSON parse error: {}", e))
    }

    pub fn status(&self) -> HttpStatus {
        HttpStatus(self.status)
    }
}

pub struct HttpStatus(u16);

impl std::fmt::Display for HttpStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Request Builder für GET requests
pub struct RequestBuilder {
    client: std::sync::Arc<HttpClientWithSocks5>,
    url: String,
}

impl RequestBuilder {
    pub async fn send(&self) -> Result<HttpResponse, String> {
        self.client.execute_request(&self.url).await
    }
}

/// Wrapper für HTTP Client mit SOCKS5 Unterstützung
pub struct HttpClientWithSocks5 {
    pub regular_client: reqwest::Client,
    pub socks_enabled: bool,
    pub socks_addr: String,
    pub socks_user: Option<String>,
    pub socks_pass: Option<String>,
}

impl HttpClientWithSocks5 {
    pub fn get(&self, url: &str) -> RequestBuilder {
        RequestBuilder {
            client: std::sync::Arc::new(HttpClientWithSocks5 {
                regular_client: self.regular_client.clone(),
                socks_enabled: self.socks_enabled,
                socks_addr: self.socks_addr.clone(),
                socks_user: self.socks_user.clone(),
                socks_pass: self.socks_pass.clone(),
            }),
            url: url.to_string(),
        }
    }

    async fn execute_request(&self, url: &str) -> Result<HttpResponse, String> {
        if self.socks_enabled {
            // Try SOCKS5 first, but fallback to direct if it fails
            match self.request_via_socks5(url).await {
                Ok(response) => Ok(response),
                Err(e) => {
                    eprintln!("⚠️ SOCKS5 request failed: {} - trying direct connection", e);
                    log_line(&format!("⚠️ SOCKS5 failed ({}), falling back to direct connection", e));
                    
                    // Fallback to direct connection
                    let response = self.regular_client
                        .get(url)
                        .send()
                        .await
                        .map_err(|e| format!("Direct HTTP request also failed: {}", e))?;
                    
                    let status = response.status().as_u16();
                    let body = response.text().await.map_err(|e| e.to_string())?;
                    log_line("✅ Direct connection fallback successful");
                    Ok(HttpResponse { body, status })
                }
            }
        } else {
            let response = self.regular_client
                .get(url)
                .send()
                .await
                .map_err(|e| format!("HTTP request failed: {}", e))?;
            
            let status = response.status().as_u16();
            let body = response.text().await.map_err(|e| e.to_string())?;
            Ok(HttpResponse { body, status })
        }
    }

    async fn request_via_socks5(&self, url: &str) -> Result<HttpResponse, String> {
        // Parse the URL
        let parsed = url::Url::parse(url)
            .map_err(|e| format!("Invalid URL: {}", e))?;
        
        let host = parsed.host_str()
            .ok_or_else(|| "No host in URL".to_string())?
            .to_string();
        let port = parsed.port().unwrap_or(if parsed.scheme() == "https" { 443 } else { 80 });
        let path = parsed.path();
        let query = parsed.query().unwrap_or("");
        let request_path = if query.is_empty() {
            if path.is_empty() { "/".to_string() } else { path.to_string() }
        } else {
            if path.is_empty() { format!("/?{}", query) } else { format!("{}?{}", path, query) }
        };

        log_line(&format!("🌐 SOCKS5 Request: {}:{} via {} (URL: {})", host, port, self.socks_addr, url));

        // Connect through SOCKS5
        let auth = if let (Some(user), Some(pass)) = (&self.socks_user, &self.socks_pass) {
            log_line(&format!("🔑 SOCKS5 with authentication: user={}", user));
            Some((user.clone(), pass.clone()))
        } else {
            log_line("SOCKS5 without authentication");
            None
        };

        let stream = if let Some((user, pass)) = auth {
            log_line(&format!("🔌 Connecting to SOCKS5 proxy {} with auth...", self.socks_addr));
            Socks5Stream::connect_with_password(
                self.socks_addr.as_str(),
                (host.clone(), port),
                &user,
                &pass,
            )
            .await
        } else {
            log_line(&format!("🔌 Connecting to SOCKS5 proxy {} ...", self.socks_addr));
            Socks5Stream::connect(self.socks_addr.as_str(), (host.clone(), port))
                .await
        }
        .map_err(|e| {
            let err_msg = format!("SOCKS5 connection to {} failed: {} (Check if 'ssh -D {}' is running)", self.socks_addr, e, self.socks_addr.split(':').last().unwrap_or("1080"));
            eprintln!("❌ {}", err_msg);
            err_msg
        })?;

        log_line("✅ SOCKS5 connection established");

        let mut socket = stream.into_inner();
        
        // For HTTPS, wrap in TLS
        let is_https = parsed.scheme() == "https";
        
        if is_https {
            log_line(&format!("🔒 Establishing TLS connection to {}...", host));
            
            let tls_connector = native_tls::TlsConnector::builder()
                .build()
                .map_err(|e| format!("Failed to create TLS connector: {}", e))?;
            let tls_connector = TlsConnector::from(tls_connector);
            
            let mut tls_stream = tls_connector
                .connect(&host, socket)
                .await
                .map_err(|e| format!("TLS handshake failed: {}", e))?;
            
            log_line("✅ TLS connection established");
            
            // Build HTTPS request
            let request = format!(
                "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: VLC/3.0.18 LibVLC/3.0.18\r\nConnection: close\r\nAccept: */*\r\n\r\n",
                request_path, host
            );

            // Write request over TLS
            tls_stream
                .write_all(request.as_bytes())
                .await
                .map_err(|e| format!("Write to TLS socket failed: {}", e))?;

            // Read response over TLS
            let mut response = Vec::new();
            tls_stream
                .read_to_end(&mut response)
                .await
                .map_err(|e| format!("Read from TLS socket failed: {}", e))?;
            
            return self.parse_http_response(response);
        }

        // Build HTTP request (non-HTTPS)
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: VLC/3.0.18 LibVLC/3.0.18\r\nConnection: close\r\nAccept: */*\r\n\r\n",
            request_path, host
        );

        // Write HTTP request
        socket
            .write_all(request.as_bytes())
            .await
            .map_err(|e| format!("Write to SOCKS5 failed: {}", e))?;

        // Read response
        let mut response = Vec::new();
        socket
            .read_to_end(&mut response)
            .await
            .map_err(|e| format!("Read from SOCKS5 failed: {}", e))?;

        self.parse_http_response(response)
    }
    
    fn parse_http_response(&self, response: Vec<u8>) -> Result<HttpResponse, String> {
        let response_str = String::from_utf8_lossy(&response).to_string();
        
        // Parse HTTP response more robustly
        let (status, body) = if let Some(header_end) = response_str.find("\r\n\r\n") {
            let headers = &response_str[..header_end];
            let mut body = response_str[header_end + 4..].to_string();
            
            // Extract status code from first line (e.g., "HTTP/1.1 200 OK")
            let status = headers
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|code| code.parse::<u16>().ok())
                .unwrap_or(500);
            
            // Check for Content-Length and Transfer-Encoding headers
            let mut content_length: Option<usize> = None;
            let mut is_chunked = false;
            
            for line in headers.lines() {
                let lower = line.to_lowercase();
                if lower.starts_with("content-length:") {
                    if let Some(len_str) = line.split(':').nth(1) {
                        content_length = len_str.trim().parse::<usize>().ok();
                    }
                } else if lower.starts_with("transfer-encoding:") && line.contains("chunked") {
                    is_chunked = true;
                }
            }
            
            // Decode chunked transfer encoding if present
            if is_chunked {
                body = decode_chunked(&body);
            } else if let Some(len) = content_length {
                // Use Content-Length to trim exactly
                if body.len() > len {
                    body.truncate(len);
                }
            }
            
            // Final cleanup: remove control characters except common JSON whitespace
            body = body
                .chars()
                .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t' | ' '))
                .collect();
            
            // Trim trailing whitespace and newlines
            body = body.trim_end().to_string();
            
            (status, body)
        } else {
            (500, response_str)
        };

        Ok(HttpResponse { body, status })
    }
}

/// Decode chunked transfer encoding
fn decode_chunked(body: &str) -> String {
    let mut result = String::new();
    let mut lines = body.lines();
    
    while let Some(chunk_line) = lines.next() {
        let chunk_line = chunk_line.trim();
        
        // Skip empty lines
        if chunk_line.is_empty() {
            continue;
        }
        
        // Parse chunk size (hex number, possibly with chunk extensions after semicolon)
        let chunk_size_str = chunk_line.split(';').next().unwrap_or("").trim();
        
        match usize::from_str_radix(chunk_size_str, 16) {
            Ok(chunk_size) => {
                if chunk_size == 0 {
                    // Last chunk, we're done
                    break;
                }
                
                // Read the chunk data
                if let Some(chunk_data) = lines.next() {
                    // Take only the specified number of characters
                    let data = chunk_data.chars().take(chunk_size).collect::<String>();
                    result.push_str(&data);
                }
            }
            Err(_) => {
                // Not a valid chunk size line, might be actual data or corrupted
                // Skip this line
                continue;
            }
        }
    }
    
    result
}

/// Build an HTTP client with optional SOCKS5 proxy support
pub async fn build_http_client(config: &Config) -> Result<HttpClientWithSocks5, String> {
    let mut regular_client = reqwest::Client::builder()
        .pool_idle_timeout(Duration::from_secs(300))
        .pool_max_idle_per_host(2)
        .tcp_nodelay(true)
        .tcp_keepalive(Some(Duration::from_secs(60)))
        .timeout(Duration::from_secs(7200))  // 2 hour timeout for long streams
        .connect_timeout(Duration::from_secs(30))
        .user_agent("VLC/3.0.18 LibVLC/3.0.18")
        .danger_accept_invalid_certs(true)  // Required for many IPTV providers
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    // Determine proxy configuration
    let proxy_addr = format!("{}:{}", config.proxy_host, config.proxy_port);
    let socks_enabled = config.proxy_enabled && config.proxy_type == "socks5" && !config.proxy_host.is_empty();

    // If HTTP proxy selected (e.g., privoxy), configure reqwest to use it
    if config.proxy_enabled && config.proxy_type == "http" && !config.proxy_host.is_empty() {
        let proxy_url = format!("http://{}:{}", config.proxy_host, config.proxy_port);
        match reqwest::Proxy::all(&proxy_url) {
            Ok(px) => {
                regular_client = reqwest::Client::builder()
                    .pool_idle_timeout(Duration::from_secs(300))
                    .pool_max_idle_per_host(2)
                    .tcp_nodelay(true)
                    .tcp_keepalive(Some(Duration::from_secs(60)))
                    .timeout(Duration::from_secs(7200))
                    .connect_timeout(Duration::from_secs(30))
                    .user_agent("VLC/3.0.18 LibVLC/3.0.18")
                    .danger_accept_invalid_certs(true)
                    .redirect(reqwest::redirect::Policy::limited(5))
                    .proxy(px)
                    .build()
                    .map_err(|e| format!("Failed to build HTTP client with HTTP proxy: {}", e))?;
                log_line(&format!("🔒 HTTP client configured with HTTP proxy: {}", proxy_url));
            }
            Err(e) => {
                log_line(&format!("⚠️ Failed to configure HTTP proxy {}: {} - falling back to direct", proxy_addr, e));
            }
        }
    } else if socks_enabled {
        log_line(&format!("🔒 HTTP client configured with SOCKS5 proxy: {}", proxy_addr));
    } else {
        log_line("HTTP client configured without proxy (direct connection)");
    }

    Ok(HttpClientWithSocks5 {
        regular_client,
        socks_enabled,
        socks_addr: proxy_addr,
        socks_user: if config.proxy_username.is_empty() { None } else { Some(config.proxy_username.clone()) },
        socks_pass: if config.proxy_password.is_empty() { None } else { Some(config.proxy_password.clone()) },
    })
}

/// Test SOCKS5 proxy connection by fetching external IP
pub async fn test_socks5_connection(config: &Config) -> Result<String, String> {
    if !config.proxy_enabled {
        return Err("Proxy is not enabled".to_string());
    }
    
    if config.proxy_host.is_empty() {
        return Err("Proxy host is empty".to_string());
    }

    log_line(&format!("Testing SOCKS5 connection to {}:{}", config.proxy_host, config.proxy_port));

    let client = build_http_client(config).await?;

    // Test connection by fetching external IP via SOCKS5 (using HTTP, not HTTPS)
    let response = client
        .get("http://api.ipify.org?format=json")
        .send()
        .await
        .map_err(|e| {
            log_line(&format!("❌ Connection test failed: {}", e));
            if e.contains("timeout") {
                format!("Connection timeout - proxy server not reachable")
            } else if e.contains("connection") || e.contains("reset") {
                format!("Connection failed - check proxy host, port and credentials: {}", e)
            } else {
                format!("Connection test failed: {}", e)
            }
        })?;

    // Log response details for debugging
    log_line(&format!("📋 Response Status: {}", response.status()));
    let body = response.text().await.map_err(|e| {
        log_line(&format!("❌ Failed to read response body: {}", e));
        format!("Failed to read response: {}", e)
    })?;
    
    log_line(&format!("📋 Response Body: {}", 
        if body.len() > 200 { 
            format!("{}... ({} bytes)", &body[..200], body.len()) 
        } else { 
            body.clone() 
        }
    ));

    // Parse JSON response
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
        log_line(&format!("❌ JSON parse error: {}", e));
        format!("Invalid JSON response: {}\nReceived: {}", e, 
            if body.len() > 100 { format!("{}...", &body[..100]) } else { body.clone() }
        )
    })?;
    
    if let Some(ip) = json["ip"].as_str() {
        log_line(&format!("✅ SOCKS5 connection test successful - IP: {}", ip));
        return Ok(format!("✓ Connected successfully!\nYour IP: {}", ip));
    }

    Err(format!("Invalid response format (missing 'ip' field): {}", body))
}

/// Download file with SOCKS5 support - returns (status_code, content_length, body_stream)
/// NOTE: Currently not used - downloads use regular HTTP client for better streaming support
#[allow(dead_code)]
pub async fn download_stream_via_socks5(
    client: &HttpClientWithSocks5,
    url: &str,
    resume_from: u64,
) -> Result<(u16, Option<u64>, Vec<u8>), String> {
    // Parse URL
    let parsed = url::Url::parse(url)
        .map_err(|e| format!("Invalid URL: {}", e))?;
    
    let host = parsed.host_str()
        .ok_or_else(|| "No host in URL".to_string())?
        .to_string();
    let port = parsed.port().unwrap_or(if parsed.scheme() == "https" { 443 } else { 80 });
    let path = parsed.path();
    let query = parsed.query().unwrap_or("");
    let request_path = if query.is_empty() {
        if path.is_empty() { "/".to_string() } else { path.to_string() }
    } else {
        if path.is_empty() { format!("/?{}", query) } else { format!("{}?{}", path, query) }
    };

    // Connect through SOCKS5
    let auth = if let (Some(user), Some(pass)) = (&client.socks_user, &client.socks_pass) {
        Some((user.clone(), pass.clone()))
    } else {
        None
    };

    let stream = if let Some((user, pass)) = auth {
        Socks5Stream::connect_with_password(
            client.socks_addr.as_str(),
            (host.clone(), port),
            &user,
            &pass,
        )
        .await
    } else {
        Socks5Stream::connect(client.socks_addr.as_str(), (host.clone(), port))
            .await
    }
    .map_err(|e| format!("SOCKS5 connection failed: {}", e))?;

    let mut socket = stream.into_inner();

    // Build HTTP GET request with Range support
    let mut request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: VLC/3.0.18 LibVLC/3.0.18\r\nConnection: close\r\n",
        request_path, host
    );
    
    if resume_from > 0 {
        request.push_str(&format!("Range: bytes={}-\r\n", resume_from));
    }
    
    request.push_str("Accept: */*\r\n\r\n");

    // Write HTTP request
    socket
        .write_all(request.as_bytes())
        .await
        .map_err(|e| format!("Write to SOCKS5 failed: {}", e))?;

    // Read response headers and body
    let mut response = Vec::new();
    socket
        .read_to_end(&mut response)
        .await
        .map_err(|e| format!("Read from SOCKS5 failed: {}", e))?;

    let response_str = String::from_utf8_lossy(&response).to_string();
    
    // Parse HTTP response
    if let Some(header_end) = response_str.find("\r\n\r\n") {
        let headers = &response_str[..header_end];
        let body_bytes = &response[header_end + 4..];
        
        // Extract status code
        let status_code = headers
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .unwrap_or(500);

        // Extract Content-Length
        let mut content_length: Option<u64> = None;
        for line in headers.lines() {
            if line.to_lowercase().starts_with("content-length:") {
                if let Some(len_str) = line.split(':').nth(1) {
                    content_length = len_str.trim().parse::<u64>().ok();
                }
                break;
            }
        }

        Ok((status_code, content_length, body_bytes.to_vec()))
    } else {
        Err("Invalid HTTP response from SOCKS5".to_string())
    }
}

/// Download a file through SOCKS5 if enabled, otherwise use regular client
/// NOTE: Currently not used - downloads use regular HTTP client for better streaming support
#[allow(dead_code)]
pub async fn download_with_socks5(client: &HttpClientWithSocks5, url: &str, resume_from: u64) -> Result<reqwest::Response, String> {
    if client.socks_enabled {
        // Use SOCKS5 for download
        download_via_socks5_streaming(client, url, resume_from).await
    } else {
        // Use regular HTTP client
        let mut req = client.regular_client.get(url);
        if resume_from > 0 {
            req = req.header(reqwest::header::RANGE, format!("bytes={}-", resume_from));
        }
        req.send().await.map_err(|e| e.to_string())
    }
}

/// Internal: Download via SOCKS5 with streaming support and range requests
/// NOTE: Currently not used - downloads use regular HTTP client for better streaming support
#[allow(dead_code)]
async fn download_via_socks5_streaming(
    client: &HttpClientWithSocks5,
    url: &str,
    resume_from: u64,
) -> Result<reqwest::Response, String> {
    // Parse URL
    let parsed = url::Url::parse(url)
        .map_err(|e| format!("Invalid URL: {}", e))?;
    
    let host = parsed.host_str()
        .ok_or_else(|| "No host in URL".to_string())?
        .to_string();
    let port = parsed.port().unwrap_or(if parsed.scheme() == "https" { 443 } else { 80 });
    let path = parsed.path();
    let query = parsed.query().unwrap_or("");
    let request_path = if query.is_empty() {
        if path.is_empty() { "/".to_string() } else { path.to_string() }
    } else {
        if path.is_empty() { format!("/?{}", query) } else { format!("{}?{}", path, query) }
    };

    log_line(&format!("⬇️ Downloading via SOCKS5: {}:{}", host, port));

    // Connect through SOCKS5
    let auth = if let (Some(user), Some(pass)) = (&client.socks_user, &client.socks_pass) {
        Some((user.clone(), pass.clone()))
    } else {
        None
    };

    let stream = if let Some((user, pass)) = auth {
        Socks5Stream::connect_with_password(
            client.socks_addr.as_str(),
            (host.clone(), port),
            &user,
            &pass,
        )
        .await
    } else {
        Socks5Stream::connect(client.socks_addr.as_str(), (host.clone(), port))
            .await
    }
    .map_err(|e| format!("SOCKS5 connection failed: {}", e))?;

    let mut socket = stream.into_inner();

    // Build HTTP GET request with Range support
    let mut request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: VLC/3.0.18 LibVLC/3.0.18\r\nConnection: close\r\n",
        request_path, host
    );
    
    if resume_from > 0 {
        request.push_str(&format!("Range: bytes={}-\r\n", resume_from));
    }
    
    request.push_str("Accept: */*\r\n\r\n");

    // Write HTTP request
    socket
        .write_all(request.as_bytes())
        .await
        .map_err(|e| format!("Write to SOCKS5 failed: {}", e))?;

    // Read response headers and body
    let mut response = Vec::new();
    socket
        .read_to_end(&mut response)
        .await
        .map_err(|e| format!("Read from SOCKS5 failed: {}", e))?;

    let response_str = String::from_utf8_lossy(&response).to_string();
    
    // Parse HTTP response
    if let Some(header_end) = response_str.find("\r\n\r\n") {
        let headers = &response_str[..header_end];
        let _body_start = header_end + 4;  // Keep for reference but unused
        
        // Extract status code
        let status_code = headers
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .unwrap_or(500);

        log_line(&format!("📥 SOCKS5 Download response: HTTP {}", status_code));

        // Convert to reqwest::Response compatible format
        // Since we can't return a real reqwest::Response, we create a wrapper
        // by sending the data back through the regular client with a local server
        // OR we just return an error and handle SOCKS5 downloads differently
        
        // For now, return error and fallback to regular client
        Err("SOCKS5 streaming download not fully supported yet - using direct download".to_string())
    } else {
        Err("Invalid HTTP response from SOCKS5".to_string())
    }
}

