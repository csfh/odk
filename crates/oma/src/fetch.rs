//! Fetching a publisher's key set: `https://<publisher>/.well-known/oma-keys` and its signature
//! (ADR 0060, section 3), with `ureq` set up as ADR 0066 says and names looked up as ADR 0077
//! says. What comes back is bytes until the root's signature over them verifies
//! (`trust::keys::verify`): TLS says where they came from, and nothing more.
//!
//! A host that does not resolve and any other failure are told apart (ADR 0060, section 2), and
//! the caller words the line, since what it says depends on whether anything is installed.

use std::ffi::{CStr, CString};
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::sync::mpsc;
use std::time::Duration;

use ureq::Agent;
use ureq::config::Config;
use ureq::http::Uri;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
use ureq::unversioned::resolver::{ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{DefaultConnector, NextTimeout};

use crate::trust::Publisher;

/// The whole fetch of one document, connection and body included (ADR 0060, section 3).
pub const TIMEOUT: Duration = Duration::from_secs(3);
/// The most of `oma-keys`, or of its signature, that is read; a longer body is a failed fetch.
pub const LIMIT: u64 = 16 * 1024;

/// Why a key set could not be fetched. Neither changes a pin.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Failure {
    /// The publisher's name does not exist, or has no address.
    #[error("`{host}` does not resolve")]
    HostNotFound { host: String },
    /// A timeout, a TLS failure, a status other than 200, a body over the limit, or any other
    /// failure: not a lost name.
    #[error("`{host}` could not be reached ({reason})")]
    Unreachable { host: String, reason: String },
}

impl Failure {
    pub fn host(&self) -> &str {
        match self {
            Failure::HostNotFound { host } | Failure::Unreachable { host, .. } => host,
        }
    }
}

/// A publisher's key set and the root's signature over it, as fetched and not yet verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySet {
    pub body: Vec<u8>,
    pub signature: Vec<u8>,
}

/// How long a release's `install.sh` or tarball may go without progress (ADR 0086).
pub const STALL: Duration = Duration::from_secs(30);

/// The one agent `odk-oma` fetches documents with (ADR 0066): HTTPS only, no redirect followed,
/// three seconds, no proxy, the system's roots, and names looked up by [`Names`] (ADR 0077).
pub fn agent() -> Agent {
    Agent::with_parts(config(Some(TIMEOUT), None), DefaultConnector::new(), Names)
}

/// The agent for a release's `install.sh` and tarball (ADR 0086): [`agent`]'s rules, except that
/// in place of three seconds in all, no step may go [`STALL`] without progress.
pub fn release_agent() -> Agent {
    Agent::with_parts(config(None, Some(STALL)), DefaultConnector::new(), Names)
}

fn config(global: Option<Duration>, stall: Option<Duration>) -> Config {
    Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .timeout_global(global)
        .timeout_resolve(stall)
        .timeout_connect(stall)
        .timeout_per_read(stall)
        .timeout_per_write(stall)
        .proxy(None)
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::Rustls)
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
}

/// Fetches `oma-keys` and `oma-keys.sig` from the publisher's host and nowhere else.
pub fn oma_keys(agent: &Agent, publisher: &Publisher) -> Result<KeySet, Failure> {
    let host = publisher.as_str();
    let get = |name: &str| get(agent, host, &format!("/.well-known/{name}"), LIMIT);
    Ok(KeySet {
        body: get("oma-keys")?,
        signature: get("oma-keys.sig")?,
    })
}

/// `https://<host><path>`, answered 200, of at most `limit` bytes; a longer body is a failed
/// fetch.
pub fn get(agent: &Agent, host: &str, path: &str, limit: u64) -> Result<Vec<u8>, Failure> {
    let failure = |e: ureq::Error| match e {
        ureq::Error::HostNotFound => Failure::HostNotFound {
            host: host.to_owned(),
        },
        e => Failure::Unreachable {
            host: host.to_owned(),
            reason: e.to_string(),
        },
    };
    let url = format!("https://{host}{path}");
    let mut response = agent.get(&url).call().map_err(failure)?;
    if response.status() != 200 {
        return Err(Failure::Unreachable {
            host: host.to_owned(),
            reason: format!("{url} answered {}", response.status()),
        });
    }
    // `ureq`'s limit fails the read that would find the end of a body exactly the limit long,
    // so the reader is given one byte more: a body of `limit` bytes is read whole, and one byte
    // more is refused.
    response
        .body_mut()
        .with_config()
        .limit(limit + 1)
        .read_to_vec()
        .map_err(failure)
}

/// Names looked up with `getaddrinfo` itself (ADR 0077). The standard library, and with it
/// `ureq`'s own resolver, gives a name that does not exist as an error with only a message,
/// which would be the "could not be reached" line; here it is `ureq::Error::HostNotFound`.
#[derive(Debug)]
struct Names;

impl Resolver for Names {
    fn resolve(
        &self,
        uri: &Uri,
        _config: &Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let host = uri
            .host()
            .ok_or_else(|| ureq::Error::BadUri(uri.to_string()))?
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned();
        let port = uri.port_u16().unwrap_or(443);
        let found = if timeout.after.is_not_happening() {
            lookup(&host, port)?
        } else {
            // `getaddrinfo` cannot be given a deadline, so it runs on a thread of its own and
            // is abandoned when the deadline passes, as `ureq`'s own resolver does.
            let (tx, rx) = mpsc::sync_channel(1);
            std::thread::spawn(move || tx.send(lookup(&host, port)).ok());
            rx.recv_timeout(*timeout.after)
                .map_err(|_| ureq::Error::Timeout(timeout.reason))??
        };
        let mut addrs = self.empty();
        for addr in found.into_iter().take(16) {
            addrs.push(addr);
        }
        if addrs.is_empty() {
            return Err(ureq::Error::HostNotFound);
        }
        Ok(addrs)
    }
}

/// The addresses of `host`, or why there are none.
fn lookup(host: &str, port: u16) -> Result<Vec<SocketAddr>, ureq::Error> {
    let name = CString::new(host).map_err(|_| ureq::Error::BadUri(host.to_owned()))?;
    let hints = libc::addrinfo {
        ai_flags: 0,
        ai_family: libc::AF_UNSPEC,
        ai_socktype: libc::SOCK_STREAM,
        ai_protocol: 0,
        ai_addrlen: 0,
        ai_addr: std::ptr::null_mut(),
        ai_canonname: std::ptr::null_mut(),
        ai_next: std::ptr::null_mut(),
    };
    let mut list: *mut libc::addrinfo = std::ptr::null_mut();
    let code = unsafe { libc::getaddrinfo(name.as_ptr(), std::ptr::null(), &hints, &mut list) };
    if code != 0 {
        return Err(gai_error(code));
    }
    let mut out = Vec::new();
    let mut at = list;
    while !at.is_null() {
        let entry = unsafe { &*at };
        if !entry.ai_addr.is_null() {
            match entry.ai_family {
                libc::AF_INET => {
                    let sa = unsafe { &*(entry.ai_addr as *const libc::sockaddr_in) };
                    let ip = Ipv4Addr::from(u32::from_be(sa.sin_addr.s_addr));
                    out.push(SocketAddr::V4(SocketAddrV4::new(ip, port)));
                }
                libc::AF_INET6 => {
                    let sa = unsafe { &*(entry.ai_addr as *const libc::sockaddr_in6) };
                    let ip = Ipv6Addr::from(sa.sin6_addr.s6_addr);
                    out.push(SocketAddr::V6(SocketAddrV6::new(
                        ip,
                        port,
                        sa.sin6_flowinfo,
                        sa.sin6_scope_id,
                    )));
                }
                _ => {}
            }
        }
        at = entry.ai_next;
    }
    unsafe { libc::freeaddrinfo(list) };
    Ok(out)
}

/// A name that does not exist, or that exists with no address, does not resolve. Anything else,
/// such as a server that did not answer, is not a lost name.
fn gai_error(code: libc::c_int) -> ureq::Error {
    match code {
        libc::EAI_NONAME | libc::EAI_NODATA => ureq::Error::HostNotFound,
        code => {
            let text = unsafe { CStr::from_ptr(libc::gai_strerror(code)) };
            ureq::Error::Io(io::Error::other(format!(
                "looking up the name failed: {}",
                text.to_string_lossy()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use ureq::unversioned::transport::{
        Buffers, ConnectionDetails, Connector, LazyBuffers, Transport,
    };

    use super::*;

    /// A server behind the connection: the raw response for each path, and every path asked
    /// for. It stands where TLS would be, so it says it is TLS.
    #[derive(Debug, Clone, Default)]
    struct Server {
        routes: Arc<Vec<(String, Vec<u8>)>>,
        asked: Arc<Mutex<Vec<String>>>,
    }

    impl Server {
        fn with(routes: &[(&str, Vec<u8>)]) -> Self {
            Self {
                routes: Arc::new(
                    routes
                        .iter()
                        .map(|(p, r)| ((*p).to_owned(), r.clone()))
                        .collect(),
                ),
                asked: Arc::default(),
            }
        }

        fn asked(&self) -> Vec<String> {
            self.asked.lock().unwrap().clone()
        }
    }

    fn response(status: &str, headers: &str, body: &[u8]) -> Vec<u8> {
        let mut r = format!(
            "HTTP/1.1 {status}\r\n{headers}content-length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        r.extend_from_slice(body);
        r
    }

    fn ok(body: &[u8]) -> Vec<u8> {
        response("200 OK", "", body)
    }

    #[derive(Debug)]
    struct Exchange {
        server: Server,
        buffers: LazyBuffers,
        sent: Vec<u8>,
        reply: Vec<u8>,
        at: usize,
    }

    impl Connector for Server {
        type Out = Exchange;

        fn connect(
            &self,
            _details: &ConnectionDetails,
            _chained: Option<()>,
        ) -> Result<Option<Exchange>, ureq::Error> {
            Ok(Some(Exchange {
                server: self.clone(),
                buffers: LazyBuffers::new(64 * 1024, 64 * 1024),
                sent: Vec::new(),
                reply: Vec::new(),
                at: 0,
            }))
        }
    }

    impl Transport for Exchange {
        fn buffers(&mut self) -> &mut dyn Buffers {
            &mut self.buffers
        }

        fn transmit_output(
            &mut self,
            amount: usize,
            _timeout: NextTimeout,
        ) -> Result<(), ureq::Error> {
            let out = self.buffers.output()[..amount].to_vec();
            self.sent.extend_from_slice(&out);
            if self.reply.is_empty() && self.sent.windows(4).any(|w| w == b"\r\n\r\n") {
                let request = String::from_utf8_lossy(&self.sent).into_owned();
                let path = request.split(' ').nth(1).unwrap_or_default().to_owned();
                self.server.asked.lock().unwrap().push(path.clone());
                self.reply = self
                    .server
                    .routes
                    .iter()
                    .find(|(p, _)| *p == path)
                    .map(|(_, r)| r.clone())
                    .unwrap_or_else(|| response("404 Not Found", "", b""));
            }
            Ok(())
        }

        fn await_input(&mut self, _timeout: NextTimeout) -> Result<bool, ureq::Error> {
            let rest = &self.reply[self.at..];
            let buf = self.buffers.input_append_buf();
            let n = rest.len().min(buf.len());
            buf[..n].copy_from_slice(&rest[..n]);
            self.buffers.input_appended(n);
            self.at += n;
            Ok(n > 0)
        }

        fn is_open(&mut self) -> bool {
            false
        }

        fn is_tls(&self) -> bool {
            true
        }
    }

    /// Names that resolve, except `gone`, which does not exist.
    #[derive(Debug)]
    struct FakeNames;

    impl Resolver for FakeNames {
        fn resolve(
            &self,
            uri: &Uri,
            _config: &Config,
            _timeout: NextTimeout,
        ) -> Result<ResolvedSocketAddrs, ureq::Error> {
            if uri.host() == Some("gone.example") {
                return Err(ureq::Error::HostNotFound);
            }
            let mut addrs = self.empty();
            addrs.push("192.0.2.1:443".parse().unwrap());
            Ok(addrs)
        }
    }

    fn fetch(server: &Server, host: &str) -> Result<KeySet, Failure> {
        let agent = Agent::with_parts(config(Some(TIMEOUT), None), server.clone(), FakeNames);
        oma_keys(&agent, &Publisher::parse(host).unwrap())
    }

    const KEYS: &str = "/.well-known/oma-keys";
    const SIG: &str = "/.well-known/oma-keys.sig";

    #[test]
    fn the_agent_is_set_up_as_adr_0066_says() {
        // The production agent is built as `install` builds it, with nothing installed first.
        let _ = agent();
        let c = config(Some(TIMEOUT), None);
        assert!(c.https_only());
        assert_eq!(c.max_redirects(), 0);
        assert_eq!(c.timeouts().global, Some(TIMEOUT));
        assert!(c.proxy().is_none(), "the environment's proxy is not read");
        assert_eq!(c.tls_config().provider(), TlsProvider::Rustls);
        assert!(matches!(
            c.tls_config().root_certs(),
            RootCerts::PlatformVerifier
        ));
    }

    #[test]
    fn a_releases_files_wait_only_for_a_stall_and_follow_no_redirect() {
        // ADR 0086: the same rules, with thirty seconds without progress in place of three in all.
        let _ = release_agent();
        let c = config(None, Some(STALL));
        assert!(c.https_only());
        assert_eq!(c.max_redirects(), 0);
        assert!(c.proxy().is_none());
        assert_eq!(c.timeouts().global, None);
        assert_eq!(c.timeouts().resolve, Some(STALL));
        assert_eq!(c.timeouts().connect, Some(STALL));
        assert_eq!(c.timeout_per_read(), Some(STALL));
    }

    #[test]
    fn a_file_is_read_up_to_its_length_and_no_further() {
        let server = Server::with(&[("/odk.tar.gz", ok(&[9; 100]))]);
        let agent = Agent::with_parts(config(None, Some(STALL)), server.clone(), FakeNames);
        assert_eq!(
            get(&agent, "odk.example", "/odk.tar.gz", 100).unwrap(),
            [9; 100]
        );
        let e = get(&agent, "odk.example", "/odk.tar.gz", 99).unwrap_err();
        assert!(matches!(e, Failure::Unreachable { .. }), "{e}");
    }

    #[test]
    fn the_key_set_and_its_signature_come_from_the_publishers_well_known_paths() {
        let server = Server::with(&[(KEYS, ok(b"{\"keys\"}")), (SIG, ok(&[7u8; 64]))]);
        let got = fetch(&server, "apps.example").unwrap();
        assert_eq!(got.body, b"{\"keys\"}");
        assert_eq!(got.signature, [7u8; 64]);
        assert_eq!(server.asked(), [KEYS, SIG]);
    }

    #[test]
    fn a_redirect_is_a_failed_fetch_and_is_not_followed() {
        let server = Server::with(&[
            (KEYS, response("301 Moved", "location: /elsewhere\r\n", b"")),
            ("/elsewhere", ok(b"{}")),
        ]);
        let e = fetch(&server, "apps.example").unwrap_err();
        assert!(matches!(e, Failure::Unreachable { .. }), "{e}");
        assert_eq!(
            server.asked(),
            [KEYS],
            "the redirect's target is never asked for"
        );
    }

    #[test]
    fn any_status_but_200_is_a_failed_fetch() {
        for status in [
            "404 Not Found",
            "500 Internal Server Error",
            "204 No Content",
        ] {
            let server = Server::with(&[(KEYS, response(status, "", b"")), (SIG, ok(b""))]);
            let e = fetch(&server, "apps.example").unwrap_err();
            assert!(matches!(e, Failure::Unreachable { .. }), "{status}: {e}");
        }
    }

    #[test]
    fn a_body_over_16_kib_fails_and_is_not_cut_short() {
        let at_limit = vec![b'a'; LIMIT as usize];
        let server = Server::with(&[(KEYS, ok(&at_limit)), (SIG, ok(&[1; 64]))]);
        assert_eq!(fetch(&server, "apps.example").unwrap().body, at_limit);

        let over = vec![b'a'; LIMIT as usize + 1];
        let server = Server::with(&[(KEYS, ok(&over)), (SIG, ok(&[1; 64]))]);
        let e = fetch(&server, "apps.example").unwrap_err();
        assert!(matches!(e, Failure::Unreachable { .. }), "{e}");
        assert_eq!(server.asked(), [KEYS], "nothing after the failure");
    }

    #[test]
    fn a_name_that_does_not_resolve_is_its_own_failure_and_nothing_is_asked() {
        let server = Server::with(&[(KEYS, ok(b"{}")), (SIG, ok(&[1; 64]))]);
        let e = fetch(&server, "gone.example").unwrap_err();
        assert_eq!(
            e,
            Failure::HostNotFound {
                host: "gone.example".into()
            }
        );
        assert_eq!(e.to_string(), "`gone.example` does not resolve");
        assert!(server.asked().is_empty());
    }

    #[test]
    fn getaddrinfo_says_a_name_does_not_exist_and_anything_else_is_not_a_lost_name() {
        for code in [libc::EAI_NONAME, libc::EAI_NODATA] {
            assert!(
                matches!(gai_error(code), ureq::Error::HostNotFound),
                "{code}"
            );
        }
        for code in [libc::EAI_AGAIN, libc::EAI_FAIL, libc::EAI_MEMORY] {
            assert!(matches!(gai_error(code), ureq::Error::Io(_)), "{code}");
        }
        // `localhost` is answered by the hosts file, not the network.
        let found = lookup("localhost", 443).unwrap();
        assert!(
            found
                .iter()
                .all(|a| a.ip().is_loopback() && a.port() == 443)
        );
        assert!(!found.is_empty());
    }
}
