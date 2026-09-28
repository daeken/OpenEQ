//! Bounded cursors for gameplay packets. No packed pointers or unchecked lengths.
pub(crate) struct Reader<'a>(pub &'a [u8]);
impl<'a> Reader<'a> {
    pub fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let bytes = self.0.get(..n)?;
        self.0 = &self.0[n..];
        Some(bytes)
    }
    pub fn skip(&mut self, n: usize) -> Option<()> {
        self.take(n).map(|_| ())
    }
    pub fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    pub fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    pub fn float(&mut self) -> Option<f32> {
        let value = f32::from_bits(self.u32()?);
        value.is_finite().then_some(value)
    }
    pub fn i32(&mut self) -> Option<i32> {
        self.u32().map(|v| v as i32)
    }
    pub fn string(&mut self, max: usize) -> Option<String> {
        let n = self.0.iter().take(max + 1).position(|b| *b == 0)?;
        let s = String::from_utf8_lossy(self.take(n)?).into_owned();
        self.skip(1)?;
        Some(s)
    }
    pub fn array(&mut self, stride: usize, max: usize) -> Option<&'a [u8]> {
        let count = self.u32()? as usize;
        if count > max {
            return None;
        }
        self.take(count.checked_mul(stride)?)
    }
    pub fn done(&self) -> bool {
        self.0.is_empty()
    }
}
pub(crate) fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}
pub(crate) fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}
