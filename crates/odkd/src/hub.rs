//! The connections: each one is sent the whole state when it opens, and again whenever it
//! changes. A client is never read from, so a client can say nothing to `odkd`.

use std::io::{ErrorKind, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;
use std::time::Duration;

/// More connections than this are refused; the application that asks again gets the line it
/// missed, because the next one that closes makes room.
pub const MAX_CLIENTS: usize = 64;
const WRITE_WAIT: Duration = Duration::from_millis(200);

#[derive(Default)]
pub struct Hub {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    line: Option<String>,
    clients: Vec<UnixStream>,
}

impl Hub {
    /// Sends `line` to every client, and keeps it for the ones that connect later. A client
    /// that cannot be written to has gone.
    pub fn publish(&self, line: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.clients.retain_mut(|client| send(client, &line));
        inner.line = Some(line);
    }

    /// Adds a client, after the line it is owed. Whether there was room.
    pub fn add(&self, mut client: UnixStream) -> bool {
        let mut inner = self.inner.lock().unwrap();
        if inner.clients.len() >= MAX_CLIENTS {
            // A client that has gone is only found out when something is written to it.
            inner.clients.retain(alive);
        }
        if inner.clients.len() >= MAX_CLIENTS {
            return false;
        }
        if let Some(line) = &inner.line
            && !send(&mut client, line)
        {
            return true;
        }
        inner.clients.push(client);
        true
    }

    #[cfg(test)]
    pub fn clients(&self) -> usize {
        self.inner.lock().unwrap().clients.len()
    }
}

fn send(client: &mut UnixStream, line: &str) -> bool {
    client.set_write_timeout(Some(WRITE_WAIT)).is_ok()
        && client.write_all(line.as_bytes()).is_ok()
        && client.write_all(b"\n").is_ok()
}

/// Whether the other end is still there and has said nothing. A client that says anything is
/// not an ODK application, and goes.
fn alive(client: &UnixStream) -> bool {
    let mut byte = 0u8;
    // SAFETY: the descriptor is open for as long as `client` is borrowed, and the buffer is one
    // byte of ours. MSG_PEEK takes nothing off the queue and MSG_DONTWAIT never blocks.
    let read = unsafe {
        libc::recv(
            client.as_raw_fd(),
            std::ptr::from_mut(&mut byte).cast(),
            1,
            libc::MSG_PEEK | libc::MSG_DONTWAIT,
        )
    };
    read < 0 && std::io::Error::last_os_error().kind() == ErrorKind::WouldBlock
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};

    fn pair() -> (UnixStream, UnixStream) {
        UnixStream::pair().unwrap()
    }

    #[test]
    fn a_client_is_sent_the_line_it_is_owed_and_each_one_after() {
        let hub = Hub::default();
        hub.publish("one".into());
        let (server, client) = pair();
        assert!(hub.add(server));
        let mut reader = BufReader::new(&client);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line, "one\n");
        hub.publish("two".into());
        line.clear();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line, "two\n");
    }

    #[test]
    fn a_client_that_has_gone_is_dropped_at_the_next_line() {
        let hub = Hub::default();
        let (server, client) = pair();
        hub.add(server);
        drop(client);
        hub.publish("x".into());
        hub.publish("y".into());
        assert_eq!(hub.clients(), 0);
    }

    #[test]
    fn at_the_limit_the_ones_that_have_gone_make_room_and_the_rest_do_not() {
        let hub = Hub::default();
        let mut kept = Vec::new();
        for _ in 0..MAX_CLIENTS {
            let (server, client) = pair();
            assert!(hub.add(server));
            kept.push(client);
        }
        let (server, _client) = pair();
        assert!(!hub.add(server), "no room");
        kept.pop();
        let (server, client) = pair();
        assert!(hub.add(server), "one has gone");
        drop(client);
        assert_eq!(hub.clients(), MAX_CLIENTS);
    }

    #[test]
    fn a_client_that_says_something_is_not_an_application_and_is_dropped_when_room_is_needed() {
        let hub = Hub::default();
        let mut talkers = Vec::new();
        for _ in 0..MAX_CLIENTS {
            let (server, mut client) = pair();
            hub.add(server);
            client.write_all(b"hello").unwrap();
            talkers.push(client);
        }
        let (server, client) = pair();
        assert!(hub.add(server));
        drop(client);
        assert_eq!(hub.clients(), 1);
    }
}
