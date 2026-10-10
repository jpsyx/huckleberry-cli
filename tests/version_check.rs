//! Software versions compare semantic precedence, never lexical strings.
use app::version::client::GitHubClient;
use app::version::{UpdateStatus, compare_versions};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

#[test]
fn release_versions_use_semantic_precedence() {
    for (installed, latest, status) in [
        ("0.9.0", "v0.10.0", UpdateStatus::UpdateAvailable),
        ("0.48.0", "v0.48.0", UpdateStatus::UpToDate),
        ("0.48.0-rc.1", "v0.48.0", UpdateStatus::UpdateAvailable),
        ("0.48.0+local", "v0.48.0+release", UpdateStatus::UpToDate),
        ("0.49.0", "v0.48.0", UpdateStatus::Ahead),
        ("0.49.0", "v1.0.0", UpdateStatus::UpdateAvailable),
    ] {
        assert_eq!(compare_versions(installed, latest).unwrap().status, status);
    }
}

#[test]
fn malformed_tags_cannot_claim_the_app_is_current() {
    for tag in ["latest", "vv0.48.0", "v0.48", "v0.48.0\nhello", "v0.048.0"] {
        assert!(compare_versions("0.48.0", tag).is_err(), "{tag}");
    }
}

fn stub(
    status: u16,
    body: &'static str,
    delay: Duration,
) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/releases/latest", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        while !request.ends_with(b"\r\n\r\n") {
            let count = socket.read(&mut buffer).unwrap();
            assert!(count > 0);
            request.extend_from_slice(&buffer[..count]);
        }
        std::thread::sleep(delay);
        let response = format!(
            "HTTP/1.1 {status} Reply\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes());
        String::from_utf8(request).unwrap()
    });
    (endpoint, worker)
}

#[tokio::test]
async fn github_check_is_public_and_identifies_the_application() {
    let (endpoint, server) = stub(
        200,
        r#"{"tag_name":"v0.48.0","draft":false,"prerelease":false}"#,
        Duration::ZERO,
    );
    let report = GitHubClient::new(&endpoint, Duration::from_secs(2))
        .unwrap()
        .check("0.47.2", false)
        .await;
    assert_eq!(report.status, UpdateStatus::UpdateAvailable);
    assert_eq!(report.latest.as_deref(), Some("0.48.0"));
    let request = server.join().unwrap().to_lowercase();
    assert!(request.starts_with("get /releases/latest http/1.1"));
    assert!(request.contains("user-agent: huckleberry-cli/0.47.2"));
    assert!(!request.contains("authorization:"));
}

#[tokio::test]
async fn unavailable_or_invalid_release_never_becomes_up_to_date() {
    for (status, body, expected) in [
        (404, "{}", UpdateStatus::NoRelease),
        (403, "{}", UpdateStatus::Unavailable),
        (429, "{}", UpdateStatus::Unavailable),
        (500, "{}", UpdateStatus::Unavailable),
        (200, "not json", UpdateStatus::Unavailable),
        (
            200,
            r#"{"tag_name":"bad","draft":false,"prerelease":false}"#,
            UpdateStatus::Unavailable,
        ),
        (
            200,
            r#"{"tag_name":"v0.48.0","draft":true,"prerelease":false}"#,
            UpdateStatus::Unavailable,
        ),
        (
            200,
            r#"{"tag_name":"v0.48.0","draft":false,"prerelease":true}"#,
            UpdateStatus::Unavailable,
        ),
    ] {
        let (endpoint, server) = stub(status, body, Duration::ZERO);
        let report = GitHubClient::new(&endpoint, Duration::from_secs(2))
            .unwrap()
            .check("0.48.0", false)
            .await;
        assert_eq!(report.status, expected, "{status}: {body}");
        assert!(report.latest.is_none());
        server.join().unwrap();
    }
}

#[tokio::test]
async fn offline_mode_does_not_open_a_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let report = GitHubClient::new(&endpoint, Duration::from_secs(1))
        .unwrap()
        .check("0.48.0", true)
        .await;
    assert_eq!(report.status, UpdateStatus::Offline);
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[tokio::test]
async fn a_slow_server_reaches_the_request_timeout() {
    let (endpoint, server) = stub(200, "{}", Duration::from_millis(100));
    let report = GitHubClient::new(&endpoint, Duration::from_millis(25))
        .unwrap()
        .check("0.48.0", false)
        .await;
    assert_eq!(report.status, UpdateStatus::Unavailable);
    assert_eq!(report.reason.as_deref(), Some("Request timed out"));
    server.join().unwrap();
}
