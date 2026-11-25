//! aflos-dc: sending our video (a camera, or a Go Live stream's screen) on
//! the voice connection, beside the mixer's audio (design/DISCORD-VIDEO.md
//! V2). A whole H264 frame is DAVE-encrypted as video, split into RTP
//! packets (RFC 6184), and each packet transport-encrypted like audio's and
//! sent on the same UDP socket. Op 12 tells Discord our video SSRCs and
//! whether the video is on.

use super::tasks::message::WsMessage;
use super::tasks::udp_rx::h264;
use super::{Cipher, CryptoState};
use discortp::rtp::MutableRtpPacket;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, RwLock};

/// H264's RTP payload type on Discord (as the receive side uses).
const H264_PT: u8 = super::connection::H264_PT;
/// The biggest RTP payload we send, under a 1,200-byte path MTU with room
/// for the header and transport encryption.
const MTU: usize = 1100;

/// Our video on one voice connection. Made by the driver when it connects
/// (`Config::video_tx`), and replaced on each reconnect.
pub struct VideoTx {
    cipher: Cipher,
    // Its own nonce counter, from a random start as audio's is.
    // ponytail: two random 32-bit counters under one key could meet in a long call; give video its own half of the space if Discord ever objects.
    crypto: CryptoState,
    udp: UdpSocket,
    dave: Arc<RwLock<Option<davey::DaveSession>>>,
    dave_version: Arc<AtomicU16>,
    ws: flume::Sender<WsMessage>,
    audio_ssrc: u32,
    video_ssrc: u32,
    rtx_ssrc: u32,
    seq: u16,
    /// Whether this connection is a Go Live stream (its video is a screen).
    screen: bool,
}

impl std::fmt::Debug for VideoTx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoTx").field("video_ssrc", &self.video_ssrc).finish_non_exhaustive()
    }
}

/// Where the driver puts the `VideoTx` once it's connected.
pub type VideoTxSlot = Arc<std::sync::Mutex<Option<VideoTx>>>;

impl VideoTx {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        cipher: Cipher,
        crypto: CryptoState,
        udp: UdpSocket,
        dave: Arc<RwLock<Option<davey::DaveSession>>>,
        dave_version: Arc<AtomicU16>,
        ws: flume::Sender<WsMessage>,
        (audio_ssrc, video_ssrc, rtx_ssrc): (u32, u32, u32),
        screen: bool,
    ) -> Self {
        Self {
            cipher,
            crypto,
            udp,
            dave,
            dave_version,
            ws,
            audio_ssrc,
            video_ssrc,
            rtx_ssrc,
            seq: rand::random(),
            screen,
        }
    }

    /// Op 12: our video is on (at `width`x`height`, `fps`) or off.
    pub fn announce(&self, on: bool, width: u32, height: u32, fps: u32) {
        let kind = if self.screen { "screen" } else { "video" };
        let stream = serde_json::json!({
            "type": kind,
            "rid": "100",
            "ssrc": self.video_ssrc,
            "rtx_ssrc": self.rtx_ssrc,
            "active": on,
            "quality": 100,
            "max_bitrate": 2_500_000,
            "max_framerate": fps,
            "max_resolution": { "type": "fixed", "width": width, "height": height },
        });
        let op = serde_json::json!({
            "op": 12,
            "d": {
                "audio_ssrc": self.audio_ssrc,
                "video_ssrc": if on { self.video_ssrc } else { 0 },
                "rtx_ssrc": if on { self.rtx_ssrc } else { 0 },
                "streams": [stream],
            },
        });
        drop(self.ws.send(WsMessage::Json(op)));
    }

    /// Sends one Annex-B frame, stamped `ts` (a 90 kHz clock).
    pub fn send_frame(&mut self, annexb: &[u8], ts: u32) -> std::io::Result<()> {
        let frame = if self.dave_version.load(Ordering::Relaxed) == 0 {
            annexb.to_vec()
        } else {
            let mut guard = self.dave.write().unwrap();
            let Some(dave) = guard.as_mut().filter(|d| d.is_ready()) else {
                return Ok(()); // DAVE on but not ready yet: nothing may go out clear.
            };
            dave.encrypt(davey::MediaType::VIDEO, davey::Codec::H264, annexb)
                .map_err(|e| std::io::Error::other(format!("DAVE: {e:?}")))?
                .into_owned()
        };
        let packets = h264::packetize(&frame, MTU);
        let last = packets.len().saturating_sub(1);
        let mode = self.crypto.kind();
        for (k, payload) in packets.iter().enumerate() {
            let (prefix, suffix) = (mode.payload_prefix_len(), mode.payload_suffix_len());
            let mut buf = vec![0u8; 12 + prefix + payload.len() + suffix + 32];
            buf[0] = 0x80;
            buf[1] = H264_PT | if k == last { 0x80 } else { 0 };
            buf[2..4].copy_from_slice(&self.seq.to_be_bytes());
            buf[4..8].copy_from_slice(&ts.to_be_bytes());
            buf[8..12].copy_from_slice(&self.video_ssrc.to_be_bytes());
            self.seq = self.seq.wrapping_add(1);
            buf[12 + prefix..12 + prefix + payload.len()].copy_from_slice(payload);
            let len = {
                let Some(mut rtp) = MutableRtpPacket::new(&mut buf[..]) else {
                    continue;
                };
                let end = self.crypto.write_packet_nonce(&mut rtp, prefix + payload.len());
                self.cipher
                    .encrypt_pkt_in_place(&mut rtp, end)
                    .map_err(|e| std::io::Error::other(format!("encrypt: {e:?}")))?;
                12 + end
            };
            self.udp.send(&buf[..len])?;
        }
        Ok(())
    }
}
