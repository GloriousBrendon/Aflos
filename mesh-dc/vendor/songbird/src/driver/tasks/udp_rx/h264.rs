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

#[cfg(test)]
mod tests {
    use super::*;

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
