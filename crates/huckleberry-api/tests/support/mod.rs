//! A stub Firestore, so the request shapes are asserted rather than assumed.
//!
//! The write methods are the part of this crate a unit test cannot reach: the
//! arithmetic they depend on is pure and tested inline, but *which* URL they
//! PATCH, with which `updateMask`, and with which body, is only visible from
//! the other end of a socket. This is that socket.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

/// One request the stub received.
#[derive(Debug, Clone)]
pub struct Recorded {
    /// `GET`, `PATCH`, `POST`.
    pub method: String,
    /// The path, without the query string.
    pub path: String,
    /// The query string, parsed. Repeated keys keep every value, which is how
    /// an `updateMask` with several field paths arrives.
    pub query: HashMap<String, Vec<String>>,
    /// The `Authorization` header, if there was one.
    pub authorization: Option<String>,
    /// The body, parsed as JSON when there was one.
    pub body: Option<serde_json::Value>,
}

impl Recorded {
    /// Every value given for one query parameter.
    #[must_use]
    pub fn query_values(&self, key: &str) -> Vec<String> {
        self.query.get(key).cloned().unwrap_or_default()
    }

    /// The update mask, sorted, so a test can compare it without depending on
    /// the order the fields happened to be built in.
    #[must_use]
    pub fn update_mask(&self) -> Vec<String> {
        let mut mask = self.query_values("updateMask.fieldPaths");
        mask.sort();
        mask
    }

    /// The document body's `fields`, decoded back to plain JSON.
    #[must_use]
    pub fn document(&self) -> serde_json::Value {
        let fields = self
            .body
            .as_ref()
            .and_then(|body| body.get("fields"))
            .and_then(serde_json::Value::as_object)
            .cloned()
            .unwrap_or_default();
        serde_json::Value::Object(huckleberry_api::firestore::value::fields_to_json(&fields))
    }
}

/// A stub server that answers a fixed script of replies.
pub struct Stub {
    /// Where to point a client.
    pub documents_url: String,
    received: Arc<Mutex<Vec<Recorded>>>,
}

impl Stub {
    /// Starts a stub that answers each request with the next reply in turn,
    /// and repeats the last one once the script runs out.
    pub async fn start(replies: Vec<serde_json::Value>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("a free port");
        let address = listener.local_addr().expect("a bound address");
        let received = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&received);

        tokio::spawn(async move {
            let mut answered = 0usize;
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let Some(request) = read_request(&mut socket).await else {
                    continue;
                };
                log.lock().await.push(request);
                let reply = replies
                    .get(answered)
                    .or_else(|| replies.last())
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                answered += 1;
                let body = reply.to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            }
        });

        Self {
            documents_url: format!("http://{address}/v1/documents"),
            received,
        }
    }

    /// Every request received so far, oldest first.
    pub async fn requests(&self) -> Vec<Recorded> {
        self.received.lock().await.clone()
    }

    /// The nth request, failing the test rather than panicking obscurely when
    /// the client made fewer than expected.
    pub async fn request(&self, index: usize) -> Recorded {
        let all = self.requests().await;
        assert!(
            index < all.len(),
            "expected at least {} requests, saw {}: {all:#?}",
            index + 1,
            all.len()
        );
        all[index].clone()
    }
}

/// Reads one HTTP request off a socket. Deliberately minimal: it understands
/// exactly what `reqwest` sends here, and nothing else.
async fn read_request(socket: &mut tokio::net::TcpStream) -> Option<Recorded> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(position) = find_header_end(&buffer) {
            break position;
        }
    };

    let head = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = head.lines();
    let mut request_line = lines.next()?.split_whitespace();
    let method = request_line.next()?.to_owned();
    let target = request_line.next()?.to_owned();

    let mut authorization = None;
    let mut content_length = 0usize;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        match name.to_ascii_lowercase().as_str() {
            "authorization" => authorization = Some(value.trim().to_owned()),
            "content-length" => content_length = value.trim().parse().unwrap_or(0),
            _ => {}
        }
    }

    let mut body_bytes = buffer[header_end + 4..].to_vec();
    while body_bytes.len() < content_length {
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            break;
        }
        body_bytes.extend_from_slice(&chunk[..read]);
    }

    let (path, query) = split_target(&target);
    Some(Recorded {
        method,
        path,
        query,
        authorization,
        body: serde_json::from_slice(&body_bytes).ok(),
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn split_target(target: &str) -> (String, HashMap<String, Vec<String>>) {
    let (path, query_string) = target.split_once('?').unwrap_or((target, ""));
    let mut query: HashMap<String, Vec<String>> = HashMap::new();
    for pair in query_string.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        query.entry(decode(key)).or_default().push(decode(value));
    }
    (decode(path), query)
}

/// Percent-decoding, enough for the parameters this crate sends.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}
