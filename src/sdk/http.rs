//! Shared blocking HTTP client policy for extension workers.
use reqwest::blocking::Client;
use std::time::Duration;

#[derive(Clone)]
pub struct HttpClient(Client);

impl HttpClient {
    pub fn new(
        connect_timeout: Duration,
        timeout: Duration,
        user_agent: &str,
    ) -> Result<Self, reqwest::Error> {
        client(connect_timeout, timeout, user_agent).map(Self)
    }
    pub fn get(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.0.get(url)
    }
    pub fn post(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.0.post(url)
    }
    pub fn inner(&self) -> &Client {
        &self.0
    }
}

pub fn client(
    connect_timeout: Duration,
    timeout: Duration,
    user_agent: &str,
) -> Result<Client, reqwest::Error> {
    Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(timeout)
        .user_agent(user_agent)
        .build()
}

#[cfg(test)]
mod tests {
    use super::client;
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };

    #[test]
    fn client_honors_proxy_and_bypass_environment() {
        const PROBE: &str = "PENINSULA_HTTP_PROXY_PROBE";
        if let Ok(direct) = std::env::var(PROBE) {
            let timeout = Duration::from_secs(4);
            let client = client(timeout, timeout, "Peninsula test").unwrap();
            for (url, expected) in [
                ("http://peninsula-proxy-test.invalid/forecast", "proxied"),
                (direct.as_str(), "direct"),
            ] {
                let response = client.get(url).send().unwrap().error_for_status().unwrap();
                assert_eq!(response.text().unwrap(), expected);
            }
            return;
        }

        let proxy = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let direct = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--exact",
                "sdk::http::tests::client_honors_proxy_and_bypass_environment",
            ])
            .env_remove("http_proxy")
            .env_remove("no_proxy")
            .env_remove("REQUEST_METHOD")
            .env(
                "HTTP_PROXY",
                format!("http://{}", proxy.local_addr().unwrap()),
            )
            .env("NO_PROXY", "127.0.0.1")
            .env(
                PROBE,
                format!("http://{}/artwork", direct.local_addr().unwrap()),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = child.spawn().unwrap();
        respond(
            &proxy,
            "GET http://peninsula-proxy-test.invalid/forecast HTTP/1.1",
            "proxied",
        );
        respond(&direct, "GET /artwork HTTP/1.1", "direct");
        let result = child.wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
    }

    fn respond(listener: &TcpListener, expected: &str, body: &str) {
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("no HTTP request received: {error}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line.trim_end(), expected);
        loop {
            line.clear();
            if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                break;
            }
        }
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    }
}
