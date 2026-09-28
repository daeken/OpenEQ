//! Packet encoding: application payloads, opcode framing and the session CRC.

/// An application-level packet: an opcode plus its payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPacket {
    pub opcode: u16,
    pub data: Vec<u8>,
}

impl AppPacket {
    pub fn new(opcode: u16, data: Vec<u8>) -> Self {
        Self { opcode, data }
    }

    pub fn empty(opcode: u16) -> Self {
        Self {
            opcode,
            data: Vec::new(),
        }
    }

    /// Encodes the opcode and payload the way EverQuest does.
    ///
    /// An opcode whose low byte is zero is prefixed with an extra `0x00`, which
    /// is what makes the two-byte form unambiguous on decode.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.data.len() + 3);
        if self.opcode & 0x00ff == 0 {
            out.push(0x00);
            out.extend_from_slice(&self.opcode.to_le_bytes());
        } else {
            out.extend_from_slice(&self.opcode.to_le_bytes());
        }
        out.extend_from_slice(&self.data);
        out
    }

    /// Decodes an application packet from a protocol payload.
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }
        let opcode = u16::from_le_bytes([data[0], data[1]]);
        if opcode == 0 {
            if data.len() == 2 {
                return Some(Self::empty(0));
            }
            if data.len() < 3 {
                return None;
            }
            let opcode = u16::from_le_bytes([data[1], data[2]]);
            return Some(Self::new(opcode, data[3..].to_vec()));
        }
        Some(Self::new(opcode, data[2..].to_vec()))
    }

    pub fn size(&self) -> usize {
        self.data.len() + if self.opcode & 0x00ff == 0 { 3 } else { 2 }
    }
}

const CRC_TABLE: [u32; 256] = build_crc_table();

const fn build_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut n = 0usize;
    while n < 256 {
        let mut c = n as u32;
        let mut bit = 0;
        while bit < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            bit += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
}

/// EQEmu's keyed CRC-32, truncated to 16 bits.
///
/// The key is mixed in little-endian byte order, then the packet bytes follow,
/// and the result is complemented.
pub fn crc16(data: &[u8], key: u32) -> u16 {
    let mut crc = 0xFFFF_FFFFu32;
    for shift in [0, 8, 16, 24] {
        let byte = ((key >> shift) & 0xff) as u8;
        crc = (crc >> 8) ^ CRC_TABLE[((crc ^ byte as u32) & 0xff) as usize];
    }
    for &byte in data {
        crc = (crc >> 8) ^ CRC_TABLE[((crc ^ byte as u32) & 0xff) as usize];
    }
    (!crc & 0xffff) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcode_roundtrip_low_byte_nonzero() {
        let packet = AppPacket::new(0x5089, vec![1, 2, 3]);
        assert_eq!(packet.encode(), vec![0x89, 0x50, 1, 2, 3]);
        assert_eq!(AppPacket::decode(&packet.encode()).unwrap(), packet);
    }

    #[test]
    fn opcode_roundtrip_low_byte_zero() {
        let packet = AppPacket::new(0x4200, vec![9]);
        assert_eq!(packet.encode(), vec![0x00, 0x00, 0x42, 9]);
        assert_eq!(AppPacket::decode(&packet.encode()).unwrap(), packet);
    }

    #[test]
    fn crc_matches_keyed_variant() {
        // Same data, different keys, must differ.
        assert_ne!(
            crc16(b"everquest", 0x1122_3344),
            crc16(b"everquest", 0x4433_2211)
        );
        assert_eq!(crc16(b"everquest", 1), crc16(b"everquest", 1));
    }
}
