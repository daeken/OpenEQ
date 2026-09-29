//! The EQEmu wire protocol.
//!
//! EverQuest's network stack is a custom reliable protocol over UDP, with three
//! application streams layered on top: **login** (account auth and server
//! selection), **world** (character select) and **zone** (the game itself).
//!
//! Layout, as spoken by the RoF2 client and by EQEmu:
//!
//! ```text
//! protocol packet:    [opcode u16 BE][sequence u16 BE][payload][crc u16 BE]
//! application packet: [opcode u16 LE][payload]
//! ```
//!
//! Application opcodes are little-endian with one quirk: an opcode whose low
//! byte is zero gains a leading `0x00` byte, so `0x4200` goes out as
//! `00 00 42`. That is why the first two bytes are ambiguous on decode and the
//! reader has to inspect the value before trusting the offset.
//!
//! The checksum is EQEmu's keyed CRC-32 truncated to 16 bits, seeded by the key
//! handed out in the session reply.

pub mod crypto;
pub mod login;
pub mod opcodes;
pub mod packet;
pub mod stream;
pub mod world;

pub use crypto::{decrypt, encrypt};
pub use opcodes::{LoginOp, SessionOp, WorldOp, ZoneOp};
pub use packet::{AppPacket, crc16};
pub use stream::{EqStream, StreamError};

pub mod death;
pub mod item_use;
pub mod session;
pub mod social;
pub mod trade;
pub mod zone;

pub mod gameplay;
pub mod inventory;

mod wire;
