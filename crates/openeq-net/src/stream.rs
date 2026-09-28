//! The reliable session layer over UDP.
//!
//! This handles session setup, sequencing, acknowledgements, retransmission and
//! reassembly of fragmented application packets. The login, world and zone
//! layers sit on top and stay unaware of the mechanics.
//!
//! Every reliable packet carries a 16-bit sequence. Wraparound is handled with
//! the usual half-space comparison: a sequence is "after" another when the
//! forward distance is less than 32768.

use std::collections::HashMap;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use tokio::sync::{Mutex, Notify, mpsc};

use crate::opcodes::SessionOp;
use crate::packet::{AppPacket, crc16};

/// Largest packet the session will put on the wire, matching EQEmu's default.
const MAX_PACKET_SIZE: usize = 512;
/// How long an unacknowledged packet waits before being resent.
const RESEND_AFTER: Duration = Duration::from_secs(2);
/// How often the ticker looks for acks to send and packets to resend.
const TICK: Duration = Duration::from_millis(100);
/// How long to wait for the session handshake reply.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Bound both silent connections and reliable packets that never get acknowledged.
const SESSION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("session setup timed out")]
    Timeout,
    #[error("unsupported session encoding")]
    Encoding,
    #[error("stream is not connected")]
    Closed,
}

/// A packet that arrived out of order and is waiting for its predecessors.
enum Pending {
    Single(Vec<u8>),
    Fragment(Vec<u8>),
}

struct Sent {
    bytes: Vec<u8>,
    sent_at: Instant,
    first_sent_at: Instant,
}

struct Inner {
    crc_key: u32,
    crc_bytes: u8,
    compressing: bool,
    out_sequence: u16,
    in_sequence: u16,
    last_ack_sent: u16,
    last_ack_received: u16,
    /// Reliable packets awaiting acknowledgement, keyed by sequence.
    sent: HashMap<u16, Sent>,
    /// Out-of-order packets, keyed by sequence.
    future: HashMap<u16, Pending>,
    resend_ack: bool,
    connected: bool,
    closing: bool,
    last_received: Instant,
    assembly: Option<(usize, Vec<u8>)>,
}

impl Inner {
    fn new(crc_key: u32, crc_bytes: u8, compressing: bool) -> Self {
        Self {
            crc_key,
            crc_bytes,
            compressing,
            out_sequence: 0,
            in_sequence: 0,
            last_ack_sent: 0,
            last_ack_received: 0xFFFF,
            sent: HashMap::new(),
            future: HashMap::new(),
            resend_ack: false,
            connected: true,
            closing: false,
            last_received: Instant::now(),
            assembly: None,
        }
    }

    fn close(&mut self) {
        self.connected = false;
        self.closing = true;
        self.sent.clear();
        self.future.clear();
        self.assembly = None;
    }
}

/// A connected reliable session.
pub struct EqStream {
    socket: Arc<UdpSocket>,
    inner: Arc<Mutex<Inner>>,
    peer: SocketAddr,
    closed: Arc<Notify>,
    incoming: mpsc::UnboundedReceiver<AppPacket>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl EqStream {
    /// Opens a socket to `peer` and completes the session handshake.
    pub async fn connect(peer: SocketAddr) -> Result<Self, StreamError> {
        let bind: SocketAddr = if peer.is_ipv4() {
            "0.0.0.0:0".parse().expect("valid bind address")
        } else {
            "[::]:0".parse().expect("valid bind address")
        };
        let socket = Arc::new(UdpSocket::bind(bind).await?);
        let connect_code = random_u32();

        // Session request: protocol version, our session id, max packet size.
        let mut request = Vec::with_capacity(14);
        request.extend_from_slice(&(SessionOp::Request as u16).to_be_bytes());
        request.extend_from_slice(&2u32.to_be_bytes());
        request.extend_from_slice(&connect_code.to_be_bytes());
        request.extend_from_slice(&(MAX_PACKET_SIZE as u32).to_be_bytes());
        socket.send_to(&request, peer).await?;

        // The reply may come from a different port than the one we addressed.
        let (inner, from) = read_session_response(&socket, connect_code).await?;

        let (tx, rx) = mpsc::unbounded_channel();
        socket.connect(from).await?;
        let mut stream = Self {
            socket,
            inner: Arc::new(Mutex::new(inner)),
            peer: from,
            closed: Arc::new(Notify::new()),
            incoming: rx,
            tasks: Vec::new(),
        };
        stream.tasks.push(stream.spawn_reader(tx));
        stream.tasks.push(stream.spawn_ticker());
        Ok(stream)
    }

    /// Sends an application packet, fragmenting it when it does not fit.
    pub async fn send(&self, packet: &AppPacket) -> Result<(), StreamError> {
        self.send_reliable(&packet.encode()).await
    }

    /// Sends an already opcode-encoded application payload.
    pub async fn send_reliable(&self, payload: &[u8]) -> Result<(), StreamError> {
        let mut outgoing: Vec<Vec<u8>> = Vec::new();
        {
            let mut inner = self.inner.lock().await;
            if !inner.connected {
                return Err(StreamError::Closed);
            }
            let overhead = 2 + 2 + inner.crc_bytes as usize + usize::from(inner.compressing);
            if payload.len() + overhead > MAX_PACKET_SIZE {
                build_fragments(&mut inner, payload, &mut outgoing);
            } else {
                let sequence = inner.out_sequence;
                let bytes = encode_reliable(&inner, SessionOp::Single, sequence, payload);
                inner.sent.insert(
                    sequence,
                    Sent {
                        bytes: bytes.clone(),
                        sent_at: Instant::now(),
                        first_sent_at: Instant::now(),
                    },
                );
                inner.out_sequence = inner.out_sequence.wrapping_add(1);
                outgoing.push(bytes);
            }
        }
        for bytes in outgoing {
            self.socket.send(&bytes).await?;
        }
        Ok(())
    }

    /// Receives the next application packet, or `None` when the stream closes.
    pub async fn recv(&mut self) -> Option<AppPacket> {
        self.incoming.recv().await
    }

    pub fn peer(&self) -> SocketAddr {
        self.peer
    }

    fn spawn_reader(
        &self,
        dispatcher: mpsc::UnboundedSender<AppPacket>,
    ) -> tokio::task::JoinHandle<()> {
        let socket = self.socket.clone();
        let inner = self.inner.clone();
        let closed = self.closed.clone();
        tokio::spawn(async move {
            let mut buffer = vec![0u8; 65536];
            loop {
                let received = tokio::select! {
                    _ = closed.notified() => break,
                    result = socket.recv(&mut buffer) => result,
                };
                let len = match received {
                    Ok(len) => len,
                    Err(error) => {
                        tracing::warn!(%error, "session receive failed");
                        break;
                    }
                };
                let mut outgoing: Vec<Vec<u8>> = Vec::new();
                let closing = {
                    let mut guard = inner.lock().await;
                    process_incoming(&mut guard, &buffer[..len], &dispatcher, &mut outgoing);
                    guard.closing
                };
                for bytes in outgoing {
                    let _ = socket.send(&bytes).await;
                }
                if closing {
                    break;
                }
            }
            // Dropping this task's dispatcher closes recv(), including when the
            // ticker wakes us after an inactivity/retransmission timeout.
            inner.lock().await.close();
        })
    }

    fn spawn_ticker(&self) -> tokio::task::JoinHandle<()> {
        let socket = self.socket.clone();
        let inner = self.inner.clone();
        let closed = self.closed.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(TICK).await;
                let mut outgoing: Vec<Vec<u8>> = Vec::new();
                let closing = {
                    let mut guard = inner.lock().await;
                    tick(&mut guard, &mut outgoing);
                    guard.closing
                };
                for bytes in outgoing {
                    let _ = socket.send(&bytes).await;
                }
                if closing {
                    closed.notify_one();
                    break;
                }
            }
        })
    }
}

impl Drop for EqStream {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

async fn read_session_response(
    socket: &UdpSocket,
    connect_code: u32,
) -> Result<(Inner, SocketAddr), StreamError> {
    let mut buffer = vec![0u8; 4096];
    let deadline = tokio::time::Instant::now() + HANDSHAKE_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(StreamError::Timeout);
        }
        let (len, from) = match tokio::time::timeout(remaining, socket.recv_from(&mut buffer)).await
        {
            Ok(result) => result?,
            Err(_) => return Err(StreamError::Timeout),
        };
        if len < 17 {
            continue;
        }
        let opcode = u16::from_be_bytes([buffer[0], buffer[1]]);
        if SessionOp::from_u16(opcode) != Some(SessionOp::Response) {
            continue;
        }
        let reply = &buffer[..len];
        let code = u32::from_be_bytes([reply[2], reply[3], reply[4], reply[5]]);
        let key = u32::from_be_bytes([reply[6], reply[7], reply[8], reply[9]]);
        let crc_bytes = reply[10];
        let pass_a = reply[11];
        let pass_b = reply[12];
        let max_packet = u32::from_be_bytes([reply[13], reply[14], reply[15], reply[16]]);

        if code != connect_code && code != connect_code.swap_bytes() {
            continue;
        }
        if !matches!(crc_bytes, 0 | 2) || pass_a > 1 || pass_b > 1 {
            return Err(StreamError::Encoding);
        }
        // EQEmu reports compression through its encode passes.
        let inner = Inner::new(key, crc_bytes, pass_a == 1 || pass_b == 1);
        tracing::debug!(
            %from,
            key = format!("{key:#010x}"),
            crc_bytes,
            max_packet,
            echoes_our_code = code == connect_code || code == connect_code.swap_bytes(),
            "session established"
        );
        return Ok((inner, from));
    }
}

fn process_incoming(
    inner: &mut Inner,
    data: &[u8],
    dispatcher: &mpsc::UnboundedSender<AppPacket>,
    outgoing: &mut Vec<Vec<u8>>,
) {
    if !inner.connected || data.len() < 2 {
        return;
    }
    let opcode = SessionOp::from_u16(u16::from_be_bytes([data[0], data[1]]));
    let prefix = if data[0] == 0 { 2 } else { 1 };
    let is_protected = !matches!(
        opcode,
        Some(SessionOp::Request | SessionOp::Response | SessionOp::OutOfSession)
    );
    let crc_len = if is_protected {
        inner.crc_bytes as usize
    } else {
        0
    };
    if data.len() < 2 + crc_len {
        return;
    }
    let body_len = data.len() - crc_len;
    if crc_len == 2 {
        let actual = u16::from_be_bytes([data[body_len], data[body_len + 1]]);
        if crc16(&data[..body_len], inner.crc_key) != actual {
            tracing::debug!("dropping packet with bad crc");
            return;
        }
    }
    let mut decoded = Vec::new();
    let body = if inner.compressing && is_protected {
        decoded.extend_from_slice(&data[..prefix]);
        match data.get(prefix).copied() {
            Some(0xa5) if body_len > prefix => {
                decoded.extend_from_slice(&data[prefix + 1..body_len])
            }
            Some(0x5a) if body_len > prefix + 1 => {
                let reader = flate2::read::ZlibDecoder::new(&data[prefix + 1..body_len]);
                if reader.take(65537).read_to_end(&mut decoded).is_err() || decoded.len() > 65536 {
                    return;
                }
            }
            _ => return,
        }
        decoded.as_slice()
    } else {
        &data[..body_len]
    };

    if process_decoded(inner, body, dispatcher, outgoing) {
        inner.last_received = Instant::now();
    }
}

fn process_decoded(
    inner: &mut Inner,
    body: &[u8],
    dispatcher: &mpsc::UnboundedSender<AppPacket>,
    outgoing: &mut Vec<Vec<u8>>,
) -> bool {
    if body.len() < 2 {
        return false;
    }
    let Some(opcode) = SessionOp::from_u16(u16::from_be_bytes([body[0], body[1]])) else {
        dispatch_application(body, dispatcher);
        return true;
    };
    match opcode {
        SessionOp::Ack => {
            if body.len() < 4 {
                return false;
            }
            {
                acknowledge(inner, u16::from_be_bytes([body[2], body[3]]));
            }
        }
        SessionOp::OutOfOrder => {
            if body.len() < 4 {
                return false;
            }
            {
                let sequence = u16::from_be_bytes([body[2], body[3]]);
                if let Some(sent) = inner.sent.get_mut(&sequence) {
                    sent.sent_at = Instant::now() - RESEND_AFTER;
                }
            }
        }
        SessionOp::Single | SessionOp::Fragment => {
            if body.len() < 4 {
                return false;
            }
            let sequence = u16::from_be_bytes([body[2], body[3]]);
            enqueue(
                inner,
                opcode,
                sequence,
                body[4..].to_vec(),
                dispatcher,
                outgoing,
            );
        }
        SessionOp::Combined => {
            let mut cursor = 2usize;
            while cursor < body.len() {
                let length = body[cursor] as usize;
                cursor += 1;
                if cursor + length > body.len() {
                    return false;
                }
                if !process_decoded(inner, &body[cursor..cursor + length], dispatcher, outgoing) {
                    return false;
                }
                if inner.closing {
                    break;
                }
                cursor += length;
            }
        }
        SessionOp::Disconnect | SessionOp::OutOfSession => inner.close(),
        SessionOp::Request
        | SessionOp::Response
        | SessionOp::KeepAlive
        | SessionOp::StatRequest
        | SessionOp::StatResponse => {}
    }
    true
}

fn enqueue(
    inner: &mut Inner,
    opcode: SessionOp,
    sequence: u16,
    payload: Vec<u8>,
    dispatcher: &mpsc::UnboundedSender<AppPacket>,
    outgoing: &mut Vec<Vec<u8>>,
) {
    // Anything at or behind the delivered position has already been handled.
    if !sequence_after(sequence, inner.in_sequence.wrapping_sub(1)) {
        inner.resend_ack = true;
        return;
    }
    let pending = match opcode {
        SessionOp::Single => Pending::Single(payload),
        _ => Pending::Fragment(payload),
    };
    inner.future.insert(sequence, pending);
    drain(inner, dispatcher, outgoing);
}

/// Delivers every packet that is now contiguous, reassembling fragments.
fn drain(
    inner: &mut Inner,
    dispatcher: &mpsc::UnboundedSender<AppPacket>,
    _outgoing: &mut Vec<Vec<u8>>,
) {
    while let Some(pending) = inner.future.remove(&inner.in_sequence) {
        inner.in_sequence = inner.in_sequence.wrapping_add(1);
        inner.resend_ack = true;
        let payload = match pending {
            Pending::Single(payload) => {
                if inner.assembly.is_some() {
                    inner.closing = true;
                    return;
                }
                Some(payload)
            }
            Pending::Fragment(payload) => {
                if let Some((total, collected)) = &mut inner.assembly {
                    if collected.len() + payload.len() > *total {
                        inner.closing = true;
                        return;
                    }
                    collected.extend_from_slice(&payload);
                } else {
                    if payload.len() < 4 {
                        inner.closing = true;
                        return;
                    }
                    let total = u32::from_be_bytes(payload[..4].try_into().unwrap()) as usize;
                    if !(2..=4 * 1024 * 1024).contains(&total) || payload.len() - 4 > total {
                        inner.closing = true;
                        return;
                    }
                    inner.assembly = Some((total, payload[4..].to_vec()));
                }
                if inner
                    .assembly
                    .as_ref()
                    .is_some_and(|(n, bytes)| *n == bytes.len())
                {
                    inner.assembly.take().map(|(_, bytes)| bytes)
                } else {
                    None
                }
            }
        };
        if let Some(payload) = payload {
            dispatch_application(&payload, dispatcher);
        }
    }
}

fn dispatch_application(data: &[u8], dispatcher: &mpsc::UnboundedSender<AppPacket>) {
    // EQ's application-combined framing can appear inside a reliable packet.
    if data.starts_with(&[0, 0x19]) {
        let mut cursor = 2;
        while cursor < data.len() {
            let mut size = data[cursor] as usize;
            cursor += 1;
            if size == 0xff {
                if cursor + 2 > data.len() {
                    return;
                }
                size = u16::from_be_bytes([data[cursor], data[cursor + 1]]) as usize;
                cursor += 2;
            }
            if size == 0 || cursor + size > data.len() {
                return;
            }
            if let Some(packet) = AppPacket::decode(&data[cursor..cursor + size]) {
                let _ = dispatcher.send(packet);
            }
            cursor += size;
        }
    } else if let Some(packet) = AppPacket::decode(data) {
        let _ = dispatcher.send(packet);
    }
}

fn acknowledge(inner: &mut Inner, sequence: u16) {
    if !sequence_after(sequence, inner.last_ack_received) {
        return;
    }
    let mut current = inner.last_ack_received.wrapping_add(1);
    loop {
        inner.sent.remove(&current);
        if current == sequence {
            break;
        }
        current = current.wrapping_add(1);
    }
    inner.last_ack_received = sequence;
}

fn tick(inner: &mut Inner, outgoing: &mut Vec<Vec<u8>>) {
    tick_at(inner, outgoing, Instant::now());
}

fn tick_at(inner: &mut Inner, outgoing: &mut Vec<Vec<u8>>, now: Instant) {
    if inner.closing
        || now.saturating_duration_since(inner.last_received) >= SESSION_TIMEOUT
        || inner
            .sent
            .values()
            .any(|sent| now.saturating_duration_since(sent.first_sent_at) >= SESSION_TIMEOUT)
    {
        inner.close();
        return;
    }
    let stale: Vec<u16> = inner
        .sent
        .iter()
        .filter(|(_, sent)| now.duration_since(sent.sent_at) > RESEND_AFTER)
        .map(|(sequence, _)| *sequence)
        .collect();
    for sequence in stale {
        if let Some(sent) = inner.sent.get_mut(&sequence) {
            sent.sent_at = now;
            outgoing.push(sent.bytes.clone());
        }
    }

    if inner.last_ack_sent != inner.in_sequence || inner.resend_ack {
        outgoing.push(encode_reliable(
            inner,
            SessionOp::Ack,
            inner.in_sequence.wrapping_sub(1),
            &[],
        ));
        inner.last_ack_sent = inner.in_sequence;
        inner.resend_ack = false;
    }
}

fn build_fragments(inner: &mut Inner, payload: &[u8], outgoing: &mut Vec<Vec<u8>>) {
    let overhead = 2 + 2 + 4 + inner.crc_bytes as usize + usize::from(inner.compressing);
    let chunk_size = MAX_PACKET_SIZE.saturating_sub(overhead).max(64);
    let mut offset = 0usize;
    let mut first = true;
    while offset < payload.len() {
        let end = (offset + chunk_size).min(payload.len());
        let mut body = Vec::with_capacity(4 + (end - offset));
        if first {
            body.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            first = false;
        }
        body.extend_from_slice(&payload[offset..end]);

        let sequence = inner.out_sequence;
        let bytes = encode_reliable(inner, SessionOp::Fragment, sequence, &body);
        inner.sent.insert(
            sequence,
            Sent {
                bytes: bytes.clone(),
                sent_at: Instant::now(),
                first_sent_at: Instant::now(),
            },
        );
        inner.out_sequence = inner.out_sequence.wrapping_add(1);
        outgoing.push(bytes);
        offset = end;
    }
}

fn encode_reliable(inner: &Inner, opcode: SessionOp, sequence: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(payload.len() + 8);
    bytes.extend_from_slice(&(opcode as u16).to_be_bytes());
    if inner.compressing {
        bytes.push(0xA5); // marker for "not actually compressed"
    }
    bytes.extend_from_slice(&sequence.to_be_bytes());
    bytes.extend_from_slice(payload);
    if inner.crc_bytes > 0 {
        let crc = crc16(&bytes, inner.crc_key);
        bytes.extend_from_slice(&crc.to_be_bytes());
    }
    bytes
}

/// Half-space wrap-aware comparison: is `candidate` ahead of `reference`?
fn sequence_after(candidate: u16, reference: u16) -> bool {
    candidate != reference && candidate.wrapping_sub(reference) < 32768
}

fn random_u32() -> u32 {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0),
    );
    hasher.finish() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restarted_server_out_of_session_is_unprotected_and_closes() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut inner = Inner::new(123, 2, true);
        process_incoming(&mut inner, &[0, 0x1d], &tx, &mut Vec::new());
        assert!(inner.closing);
        assert!(!inner.connected);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn only_valid_keepalive_resets_inactivity_deadline() {
        let (tx, _) = mpsc::unbounded_channel();
        let mut inner = Inner::new(123, 2, true);
        let old = Instant::now() - SESSION_TIMEOUT;
        inner.last_received = old;
        let mut wire = vec![0, 6, 0xa5];
        wire.extend(crc16(&wire, 123).to_be_bytes());
        let mut corrupt = wire.clone();
        corrupt[3] ^= 1;
        process_incoming(&mut inner, &corrupt, &tx, &mut Vec::new());
        assert_eq!(inner.last_received, old);
        process_incoming(&mut inner, &[0, 6], &tx, &mut Vec::new());
        assert_eq!(inner.last_received, old);
        process_incoming(&mut inner, &wire, &tx, &mut Vec::new());
        assert!(inner.last_received > old);
        let mut outgoing = Vec::new();
        let deadline = inner.last_received + SESSION_TIMEOUT;
        tick_at(&mut inner, &mut outgoing, deadline - TICK);
        assert!(inner.connected);
        tick_at(&mut inner, &mut outgoing, deadline);
        assert!(!inner.connected);
        assert!(inner.closing);
    }

    #[test]
    fn retransmissions_expire_even_when_peer_sends_keepalives() {
        let mut inner = Inner::new(123, 2, true);
        let start = Instant::now();
        inner.sent.insert(
            0,
            Sent {
                bytes: vec![1, 2],
                sent_at: start,
                first_sent_at: start,
            },
        );
        let mut outgoing = Vec::new();
        for seconds in [3, 6, 9, 29] {
            let now = start + Duration::from_secs(seconds);
            inner.last_received = now;
            tick_at(&mut inner, &mut outgoing, now);
            assert!(inner.connected);
            assert_eq!(inner.sent[&0].first_sent_at, start);
        }
        assert!(!outgoing.is_empty());
        outgoing.clear();
        inner.last_received = start + SESSION_TIMEOUT;
        tick_at(&mut inner, &mut outgoing, start + SESSION_TIMEOUT);
        assert!(!inner.connected);
        assert!(inner.sent.is_empty());
        assert!(outgoing.is_empty());
    }

    #[tokio::test]
    async fn timeout_wakes_reader_and_closes_application_channel() {
        let peer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let address = peer.local_addr().unwrap();
        socket.connect(address).await.unwrap();
        let (tx, rx) = mpsc::unbounded_channel();
        let mut inner = Inner::new(123, 2, true);
        inner.last_received = Instant::now() - SESSION_TIMEOUT;
        let mut stream = EqStream {
            socket,
            inner: Arc::new(Mutex::new(inner)),
            peer: address,
            closed: Arc::new(Notify::new()),
            incoming: rx,
            tasks: Vec::new(),
        };
        stream.tasks.push(stream.spawn_reader(tx));
        stream.tasks.push(stream.spawn_ticker());
        assert!(
            tokio::time::timeout(Duration::from_secs(1), stream.recv())
                .await
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            stream.send(&AppPacket::empty(0x7dfc)).await,
            Err(StreamError::Closed)
        ));
    }

    #[test]
    fn compressed_combined_carries_reliable_and_unreliable_updates() {
        use std::io::Write;
        let profile = AppPacket::new(0x6506, vec![7; 80]);
        let movement = AppPacket::new(0x7dfc, vec![9; 24]);
        let sender = Inner::new(0, 0, false);
        let reliable = encode_reliable(&sender, SessionOp::Single, 0, &profile.encode());
        let mut body = vec![reliable.len() as u8];
        body.extend(reliable);
        body.push(movement.encode().len() as u8);
        body.extend(movement.encode());
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        z.write_all(&body).unwrap();
        let mut wire = vec![0, 3, 0x5a];
        wire.extend(z.finish().unwrap());
        wire.extend(crc16(&wire, 123).to_be_bytes());
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut receiver = Inner::new(123, 2, true);
        process_incoming(&mut receiver, &wire, &tx, &mut Vec::new());
        assert_eq!(rx.try_recv().unwrap(), profile);
        assert_eq!(rx.try_recv().unwrap(), movement);
        wire[4] ^= 1;
        process_incoming(&mut receiver, &wire, &tx, &mut Vec::new());
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn standalone_unreliable_packet_uses_one_byte_compression_prefix() {
        let packet = AppPacket::new(0x7dfc, vec![42; 24]);
        let mut wire = packet.encode();
        wire.insert(1, 0xa5);
        wire.extend(crc16(&wire, 55).to_be_bytes());
        let (tx, mut rx) = mpsc::unbounded_channel();
        process_incoming(&mut Inner::new(55, 2, true), &wire, &tx, &mut Vec::new());
        assert_eq!(rx.try_recv().unwrap(), packet);
    }

    #[test]
    fn large_profile_acks_each_fragment_before_completion() {
        let mut sender = Inner::new(123, 2, true);
        let packet = AppPacket::new(0x6506, vec![8; 40000]);
        let mut wire = Vec::new();
        build_fragments(&mut sender, &packet.encode(), &mut wire);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut receiver = Inner::new(123, 2, true);
        for (i, bytes) in wire.iter().enumerate() {
            assert!(bytes.len() <= MAX_PACKET_SIZE);
            process_incoming(&mut receiver, bytes, &tx, &mut Vec::new());
            assert_eq!(receiver.in_sequence, (i + 1) as u16);
            if i + 1 < wire.len() {
                assert!(rx.try_recv().is_err());
            }
        }
        assert_eq!(rx.try_recv().unwrap(), packet);
    }

    #[test]
    fn sequence_comparison_wraps() {
        assert!(sequence_after(1, 0));
        assert!(!sequence_after(0, 1));
        assert!(sequence_after(0, 0xFFFF));
        assert!(!sequence_after(0xFFFF, 0));
    }

    #[test]
    fn fragments_reassemble_in_order() {
        let mut sender = Inner::new(0, 0, false);
        let packet = AppPacket::new(0x5089, (0..3000u32).map(|value| value as u8).collect());
        let payload = packet.encode();
        let mut outgoing = Vec::new();
        build_fragments(&mut sender, &payload, &mut outgoing);
        assert!(outgoing.len() > 1, "payload should have been fragmented");

        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut receiver = Inner::new(0, 0, false);
        let mut acks = Vec::new();
        for bytes in &outgoing {
            process_incoming(&mut receiver, bytes, &tx, &mut acks);
        }
        assert_eq!(rx.try_recv().expect("packet delivered"), packet);
    }

    #[test]
    fn out_of_order_fragments_still_reassemble() {
        let mut sender = Inner::new(0, 0, false);
        let packet = AppPacket::new(0x6506, (0..3000u32).map(|value| value as u8).collect());
        let payload = packet.encode();
        let mut outgoing = Vec::new();
        build_fragments(&mut sender, &payload, &mut outgoing);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut receiver = Inner::new(0, 0, false);
        let mut acks = Vec::new();
        // Deliver everything except the first fragment, then the first.
        for bytes in outgoing.iter().skip(1) {
            process_incoming(&mut receiver, bytes, &tx, &mut acks);
        }
        assert!(rx.try_recv().is_err(), "nothing should be delivered yet");
        process_incoming(&mut receiver, &outgoing[0], &tx, &mut acks);
        assert_eq!(rx.try_recv().unwrap(), packet);
    }

    #[test]
    fn single_packets_deliver_in_order() {
        let mut sender = Inner::new(0, 0, false);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut receiver = Inner::new(0, 0, false);
        let mut acks = Vec::new();

        let mut frames = Vec::new();
        for opcode in [0x5089u16, 0x6506, 0x1795] {
            let packet = AppPacket::new(opcode, vec![opcode as u8]);
            let mut out = Vec::new();
            let sequence = sender.out_sequence;
            out.push(encode_reliable(
                &sender,
                SessionOp::Single,
                sequence,
                &packet.encode(),
            ));
            sender.out_sequence = sender.out_sequence.wrapping_add(1);
            frames.push(out.pop().unwrap());
        }

        process_incoming(&mut receiver, &frames[2], &tx, &mut acks);
        assert!(rx.try_recv().is_err());
        process_incoming(&mut receiver, &frames[0], &tx, &mut acks);
        assert_eq!(rx.try_recv().unwrap().opcode, 0x5089);
        process_incoming(&mut receiver, &frames[1], &tx, &mut acks);
        assert_eq!(rx.try_recv().unwrap().opcode, 0x6506);
        assert_eq!(rx.try_recv().unwrap().opcode, 0x1795);
    }
}
