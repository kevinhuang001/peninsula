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

/// Run an asynchronous HTTP transaction on an SDK worker. Cancellation drops
/// the transaction future, including any requests still reading headers or bodies.
pub fn run_cancellable<T>(
    cancellation: &crate::sdk::tasks::CancellationToken,
    transaction: impl std::future::Future<Output = T>,
) -> std::io::Result<Option<T>> {
    use futures_util::future::{select, Either};
    if cancellation.is_cancelled() {
        return Ok(None);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(async {
        let canceled = async {
            while !cancellation.is_cancelled() {
                tokio::time::sleep(Duration::from_millis(15)).await;
            }
        };
        match select(Box::pin(canceled), Box::pin(transaction)).await {
            Either::Left(_) => None,
            Either::Right((value, _)) if !cancellation.is_cancelled() => Some(value),
            Either::Right(_) => None,
        }
    });
    runtime.shutdown_timeout(Duration::from_millis(50));
    Ok(result)
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
    fn cancelling_drops_a_stalled_http_transaction() {
        use super::run_cancellable;
        use crate::sdk::tasks::CancellationToken;
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let token = CancellationToken::default();
        let worker_token = token.clone();
        let work = std::thread::spawn(move || {
            run_cancellable(&worker_token, async {
                reqwest::Client::builder()
                    .no_proxy()
                    .build()
                    .unwrap()
                    .get(format!("http://{address}/stalled"))
                    .send()
                    .await
            })
            .unwrap()
        });
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut request = String::new();
        reader.read_line(&mut request).unwrap();
        assert!(request.starts_with("GET /stalled HTTP/1.1"));
        token.cancel();
        assert!(work.join().unwrap().is_none());
    }

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
