use bitvec::prelude::*;
use hkdf::HkdfExtract;
use rand::Rng;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const BIP39_ENGLISH: &str = include_str!("../resources/BIP39English.txt");

/// List of BIP39 words. There is a test that checks that this list has correct
/// length, so there is no need to check when using this in the tool.
pub fn bip39_words() -> impl Iterator<Item = &'static str> { BIP39_ENGLISH.split_whitespace() }

/// Inverse mapping to the implicit mapping in bip39_words. Maps word to its
/// index in the list. This allows to quickly test membership and convert words
/// to their index.
pub fn bip39_map() -> HashMap<&'static str, usize> { bip39_words().zip(0..).collect() }

/// Rerandomize given list of words using system randomness and HKDF extractor.
/// The input can be an arbitrary slice of strings.
/// The output is a valid BIP39 sentence with 24 words.
pub fn rerandomize_bip39(
    input_words: &[String],
    bip_word_list: &[&str],
) -> Result<Vec<String>, String> {
    // Get randomness from system.
    // Fill array with 256 random bytes, corresponding to 2048 bits.
    let mut system_randomness = [0u8; 256];
    rand::thread_rng().fill(&mut system_randomness[..]);

    // Combine both sources of randomness using HKDF extractor.
    // For added security, use pseudorandom salt.
    let salt = Sha256::digest(b"concordium-key-generation-tool-version-1");
    let mut extract_ctx = HkdfExtract::<Sha256>::new(Some(&salt));

    // First add all words separated by " " in input_words to key material.
    // Separation ensures word boundaries are preserved
    // to prevent different word lists from resulting in same string.
    for word in input_words {
        extract_ctx.input_ikm(word.as_bytes());
        extract_ctx.input_ikm(b" ");
    }

    // Now add system randomness to key material
    extract_ctx.input_ikm(&system_randomness);

    // Finally extract random key
    let (prk, _) = extract_ctx.finalize();

    // convert raw randomness to BIP39 word sentence
    let output_words = bytes_to_bip39(&prk, bip_word_list)?;

    Ok(output_words)
}

/// Convert given byte array to valid BIP39 sentence.
/// Bytes must contain {16, 20, 24, 28, 32} bytes corresponding to
/// {128, 160, 192, 224, 256} bits.
/// This uses the method described at <https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki>.
pub fn bytes_to_bip39(bytes: &[u8], bip_word_list: &[&str]) -> Result<Vec<String>, String> {
    let ent_len = 8 * bytes.len(); // input is called entropy in BIP39
    match ent_len {
        128 | 160 | 192 | 224 | 256 => (),
        _ => {
            return Err(
                "The number of bytes to be converted to a BIP39 sentence must be in {16, 20, 24, \
                 28, 32}."
                    .to_string(),
            )
        }
    };

    // checksum length is ent_len / 32
    let cs_len = ent_len / 32;

    // checksum is first cs_len bits of SHA256(bytes)
    // first compute hash of bytes
    let hash = Sha256::digest(bytes);

    // convert hash from byte vector to bit vector
    let hash_bits = BitVec::<u8, Msb0>::from_slice(&hash);

    // convert input bytes from byte vector to bit vector
    let mut random_bits = BitVec::<u8, Msb0>::from_slice(bytes);

    // append the first cs_len bits of hash_bits to the end of random_bits
    for i in 0..cs_len {
        random_bits.push(hash_bits[i]);
    }

    // go over random_bits in chunks of 11 bits and convert those to words
    let mut vec = Vec::<String>::new();
    let random_iter = random_bits.chunks(11);
    for chunk in random_iter {
        let idx = chunk.iter().fold(0, |acc, b| acc << 1 | *b as usize); // convert chunk to integer
        vec.push(bip_word_list[idx].to_string());
    }

    Ok(vec)
}

/// Verify whether the given vector of words constitutes a valid BIP39 sentence.
pub fn verify_bip39(word_vec: &[String], bip_word_map: &HashMap<&str, usize>) -> bool {
    // check that word_vec contains allowed number of words
    match word_vec.len() {
        12 | 15 | 18 | 21 | 24 => (),
        _ => return false,
    };

    // convert word vector to bits
    let mut bit_vec = BitVec::<u8, Msb0>::new();
    for word in word_vec {
        match bip_word_map.get(word.as_str()) {
            Some(idx) => {
                let word_bits = BitVec::<u16, Msb0>::from_element(*idx as u16);
                // There are 2048 words in the BIP39 list, which can be represented using 11
                // bits. Thus, the first 5 bits of word_bits are 0. Remove those leading zeros
                // and add the remaining ones to bit_bec.
                bit_vec.extend_from_bitslice(&word_bits[5..]);
            }
            None => return false, // not valid if it contains invalid word
        };
    }

    // Valid sentence consists of initial entropy of length ent_len plus
    // checksum of length ent_len/32. Hence, ent_len * 33/32 = bit_vec.len().
    // Note that bit_vec.len() is always a multiple of 33 because 11 bits
    // are added for each word and all allowed word counts are multiples of 3.
    let ent_len = 32 * bit_vec.len() / 33;

    // split bits after ent_len off. These correspond to the checksum.
    let checksum = bit_vec.split_off(ent_len);

    // checksum is supposed to be first cs_len bits of SHA256(entropy)
    let hash = Sha256::digest(bit_vec.into_vec());

    // convert hash from byte vector to bit vector
    let hash_bits = BitVec::<u8, Msb0>::from_slice(&hash);

    // sentence is valid if checksum equals fist ent_len/32 bits of hash
    checksum == hash_bits[0..ent_len / 32]
}
