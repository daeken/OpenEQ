//! The login handshake's block cipher.
//!
//! Credentials are encrypted with DES in CBC mode using an all-zero key and IV
//! (EQEmu calls this `eqcrypt_block`). It is not a security boundary: it only
//! keeps the password out of casual view, and EQEmu still accepts it.

use des::Des;
use des::cipher::{Block, BlockCipherDecrypt, BlockCipherEncrypt, Key, KeyInit};

const ZERO: [u8; 8] = [0; 8];

/// Encrypts a credential block, zero-padding to a multiple of 8 bytes.
pub fn encrypt(plain: &[u8]) -> Vec<u8> {
    let mut buffer = plain.to_vec();
    let remainder = buffer.len() % 8;
    if remainder != 0 {
        buffer.resize(buffer.len() + 8 - remainder, 0);
    }
    let key: Key<Des> = ZERO.into();
    let cipher = Des::new(&key);
    let mut previous = ZERO;
    for block in buffer.chunks_exact_mut(8) {
        for (byte, chain) in block.iter_mut().zip(previous.iter()) {
            *byte ^= *chain;
        }
        let mut encrypted = Block::<Des>::default();
        encrypted.copy_from_slice(block);
        cipher.encrypt_block(&mut encrypted);
        block.copy_from_slice(&encrypted);
        previous.copy_from_slice(block);
    }
    buffer
}

/// Decrypts a credential block. Returns `None` if the length is not a whole
/// number of DES blocks.
pub fn decrypt(ciphertext: &[u8]) -> Option<Vec<u8>> {
    if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(8) {
        return None;
    }
    let mut buffer = ciphertext.to_vec();
    let key: Key<Des> = ZERO.into();
    let cipher = Des::new(&key);
    let mut previous = ZERO;
    for block in buffer.chunks_exact_mut(8) {
        let mut ciphertext_block = Block::<Des>::default();
        ciphertext_block.copy_from_slice(block);
        let mut plain_block = ciphertext_block;
        cipher.decrypt_block(&mut plain_block);
        for (byte, chain) in plain_block.iter_mut().zip(previous.iter()) {
            *byte ^= *chain;
        }
        block.copy_from_slice(&plain_block);
        previous.copy_from_slice(&ciphertext_block);
    }
    Some(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let plain = b"username\0password\0";
        let encrypted = encrypt(plain);
        assert_eq!(encrypted.len() % 8, 0);
        // Decryption returns the whole padded block; readers stop at the NUL.
        assert_eq!(&decrypt(&encrypted).unwrap()[..plain.len()], plain);
    }

    #[test]
    fn pads_partial_blocks() {
        let encrypted = encrypt(b"abc");
        assert_eq!(encrypted.len(), 8);
        assert_eq!(&decrypt(&encrypted).unwrap()[..3], b"abc");
    }
}
