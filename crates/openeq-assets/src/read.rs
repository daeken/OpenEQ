//! A small little-endian cursor shared by the format parsers.
//!
//! Every read is bounds-checked and reports the offset it failed at, which
//! makes malformed asset files much easier to diagnose.

use crate::{Error, Result};

pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    origin: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            origin: 0,
        }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn set_pos(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        if self.pos + len > self.data.len() {
            return Err(Error::Truncated {
                offset: self.origin + self.pos,
                needed: len,
                available: self.remaining(),
            });
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    pub fn skip(&mut self, len: usize) -> Result<()> {
        self.take(len).map(|_| ())
    }

    pub fn align4(&mut self) -> Result<()> {
        let aligned = (self.pos + 3) & !3;
        if aligned > self.data.len() {
            return Err(Error::Truncated {
                offset: self.origin + self.pos,
                needed: aligned - self.pos,
                available: self.remaining(),
            });
        }
        self.pos = aligned;
        Ok(())
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn i8(&mut self) -> Result<i8> {
        Ok(self.u8()? as i8)
    }

    pub fn u16(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn i16(&mut self) -> Result<i16> {
        Ok(self.u16()? as i16)
    }

    pub fn u32(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }

    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }

    pub fn vec2(&mut self) -> Result<[f32; 2]> {
        Ok([self.f32()?, self.f32()?])
    }

    pub fn vec3(&mut self) -> Result<[f32; 3]> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }

    /// Reads a signed element count, rejecting values that cannot possibly fit
    /// in the remaining data. Corrupt assets otherwise trigger huge
    /// allocations before the bounds check fires.
    pub fn bounded_count(&mut self) -> Result<usize> {
        let raw = self.i32()?;
        if raw < 0 || raw as usize > self.data.len() {
            return Err(Error::Format(format!(
                "implausible element count {raw} at offset {}",
                self.origin + self.pos
            )));
        }
        Ok(raw as usize)
    }
}
