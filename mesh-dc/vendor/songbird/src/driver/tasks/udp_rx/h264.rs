//! mesh-dc: H264 over RTP (RFC 6184) back into whole Annex-B frames. DAVE
//! encrypts a whole encoded frame before it's split into packets, so a
//! frame must be put together again before it can be decrypted. A frame
//! with a packet missing is dropped: the next keyframe recovers.

const START: [u8; 4] = [0, 0, 0, 1];

#[derive(Debug, Default)]
pub(crate) struct H264Assembler {
    frame: Vec<u8>,
    /// The timestamp of the frame being gathered.
    ts: Option<u32>,
    next_seq: Option<u16>,
    broken: bool,
}

impl H264Assembler {
    /// One RTP payload; a whole frame once its last packet (the marker) is in.
    pub(crate) fn push(&mut self, seq: u16, ts: u32, marker: bool, payload: &[u8]) -> Option<Vec<u8>> {
        if self.ts != Some(ts) {
            // A new frame: a gap just before it may have taken its start.
            self.broken = self.next_seq.is_some_and(|n| n != seq);
            self.frame.clear();
            self.ts = Some(ts);
        } else if self.next_seq != Some(seq) {
            self.broken = true;
        }
        self.next_seq = Some(seq.wrapping_add(1));
        match payload.first().map(|b| b & 0x1f) {
            Some(1..=23) => {
                self.frame.extend_from_slice(&START);
                self.frame.extend_from_slice(payload);
            },
            // STAP-A: several NAL units, each after a 2-byte size.
            Some(24) => {
                let mut i = 1;
                while i + 2 <= payload.len() {
                    let size = usize::from(u16::from_be_bytes([payload[i], payload[i + 1]]));
                    let Some(nal) = payload.get(i + 2..i + 2 + size) else {
                        self.broken = true;
                        break;
                    };
                    self.frame.extend_from_slice(&START);
                    self.frame.extend_from_slice(nal);
                    i += 2 + size;
                }
            },
            // FU-A: one NAL unit split over packets.
            Some(28) if payload.len() >= 2 => {
                let (indicator, header) = (payload[0], payload[1]);
                if header & 0x80 != 0 {
                    self.frame.extend_from_slice(&START);
                    self.frame.push((indicator & 0xe0) | (header & 0x1f));
                } else if self.frame.is_empty() {
                    // A middle with no start: its start was lost.
                    self.broken = true;
                }
                self.frame.extend_from_slice(&payload[2..]);
            },
            _ => {},
        }
        if !marker {
            return None;
        }
        self.ts = None;
        let frame = std::mem::take(&mut self.frame);
        (!self.broken && !frame.is_empty()).then_some(frame)
    }
}

/// mesh-dc: an Annex-B frame's NAL units (start codes of 3 or 4 bytes).
pub(crate) fn nal_units(frame: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= frame.len() {
        if frame[i] == 0 && frame[i + 1] == 0 && frame[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::new();
    for (k, &start) in starts.iter().enumerate() {
        let mut end = starts.get(k + 1).map_or(frame.len(), |next| next - 3);
        // A 4-byte start code's leading zero belongs to it, not to the unit.
        if k + 1 < starts.len() && end > start && frame[end - 1] == 0 {
            end -= 1;
        }
        if end > start {
            out.push(&frame[start..end]);
        }
    }
    out
}

/// mesh-dc: an Annex-B frame as RTP payloads (RFC 6184): a unit that fits
/// in `mtu` goes whole, a bigger one in FU-A pieces. The frame's last
/// payload is the last here (its packet carries the marker).
pub(crate) fn packetize(frame: &[u8], mtu: usize) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for nal in nal_units(frame) {
        if nal.len() <= mtu {
            out.push(nal.to_vec());
            continue;
        }
        let (indicator, kind) = ((nal[0] & 0xe0) | 28, nal[0] & 0x1f);
        let pieces: Vec<&[u8]> = nal[1..].chunks(mtu - 2).collect();
        for (k, piece) in pieces.iter().enumerate() {
            let mut header = kind;
            if k == 0 {
                header |= 0x80;
            }
            if k + 1 == pieces.len() {
                header |= 0x40;
            }
            let mut payload = vec![indicator, header];
            payload.extend_from_slice(piece);
            out.push(payload);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame split into packets and put back together is the same frame,
    /// with big units in FU-A pieces no bigger than the MTU.
    #[test]
    fn packetize_then_assemble() {
        let big: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8 | 1).collect();
        let mut frame = vec![0, 0, 0, 1, 0x67, 1, 2, 0, 0, 0, 1, 0x68, 3, 0, 0, 1, 0x65];
        frame.extend_from_slice(&big);
        let packets = packetize(&frame, 1100);
        assert!(packets.iter().all(|p| p.len() <= 1100));
        assert_eq!(packets[0], [0x67, 1, 2]);
        assert_eq!(packets[2][0] & 0x1f, 28, "the IDR goes in FU-A");
        let mut a = H264Assembler::default();
        let last = packets.len() - 1;
        let mut back = None;
        for (k, p) in packets.iter().enumerate() {
            back = a.push(k as u16, 9000, k == last, p);
        }
        let back = back.expect("a whole frame");
        assert_eq!(nal_units(&back), nal_units(&frame));
    }

    #[test]
    fn single_stap_and_fu() {
        let mut a = H264Assembler::default();
        // STAP-A with an SPS (7) and a PPS (8), then an IDR (5) in two FU-As.
        let stap = [24, 0, 2, 0x67, 1, 0, 2, 0x68, 2];
        assert_eq!(a.push(1, 900, false, &stap), None);
        assert_eq!(a.push(2, 900, false, &[0x7c, 0x85, 10, 11]), None);
        let frame = a.push(3, 900, true, &[0x7c, 0x45, 12]).unwrap();
        assert_eq!(frame, [0, 0, 0, 1, 0x67, 1, 0, 0, 0, 1, 0x68, 2, 0, 0, 0, 1, 0x65, 10, 11, 12]);
        // A single NAL unit frame.
        assert_eq!(a.push(4, 1800, true, &[0x41, 7]).unwrap(), [0, 0, 0, 1, 0x41, 7]);
    }

    #[test]
    fn a_lost_packet_drops_its_frame_only() {
        let mut a = H264Assembler::default();
        assert_eq!(a.push(1, 900, false, &[0x7c, 0x85, 1]), None);
        assert_eq!(a.push(3, 900, true, &[0x7c, 0x45, 3]), None, "2 is missing");
        assert!(a.push(4, 1800, true, &[0x41, 7]).is_some(), "the next frame is whole");
        assert_eq!(a.push(6, 2700, true, &[0x41, 8]), None, "a gap before a frame breaks it");
    }
}
