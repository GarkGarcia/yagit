//! HTML Escaping
//!
//! Stolen from pulldown-cmark-escape
//! <https://github.com/pulldown-cmark/pulldown-cmark/>

use std::fmt::{self, Display};

static ESCAPE_TABLE: [Option<&str>; 256] = create_html_escape_table();

/// A wrapper for HTML-escaped strings
pub struct Escaped<'a>(pub &'a str);

// Stolen from pulldown-cmark-escape
impl Display for Escaped<'_> {
  #[cfg(target_arch = "x86_64")]
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    // The SIMD accelerated code uses the PSHUFB instruction, which is part
    // of the SSSE3 instruction set. Further, we can only use this code if
    // the buffer is at least one VECTOR_SIZE in length to prevent reading
    // out of bounds
    if is_x86_feature_detected!("ssse3") && self.0.len() >= simd::VECTOR_SIZE {
      simd::fmt_escaped_html(self.0, f)
    } else {
      fmt_escaped_html_scalar(self.0, f)
    }
  }

  #[cfg(not(target_arch = "x86_64"))]
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt_escaped_html_scalar(self.0, f)
  }
}

// Stolen from pulldown-cmark-escape
fn fmt_escaped_html_scalar(
  s: &str,
  f: &mut fmt::Formatter<'_>
) -> fmt::Result {
  let bytes = s.as_bytes();
  let mut mark = 0;
  let mut i = 0;

  while i < s.len() {
    let next_escaped = bytes[i..]
      .iter()
      .enumerate()
      .find_map(|(offset, c)| Some((offset, ESCAPE_TABLE[*c as usize]?)));

    if let Some((offset, escape_seq)) = next_escaped {
      i += offset;
      f.write_str(&s[mark..i])?;
      f.write_str(escape_seq)?;

      i += 1;
      mark = i; // all escaped characters are ASCII
    } else {
      break;
    }
  }

  f.write_str(&s[mark..])
}

const fn create_html_escape_table() -> [Option<&'static str>; 256] {
  let mut table = [None; 256];
  table[b'<'  as usize] = Some("&lt;");
  table[b'>'  as usize] = Some("&gt;");
  table[b'&'  as usize] = Some("&amp;");
  table[b'"'  as usize] = Some("&quot;");
  table[b'\'' as usize] = Some("&apos;");
  table
}

// stolen from pulldown-cmark-escape
#[cfg(target_arch = "x86_64")]
mod simd {
  use std::{
    arch::x86_64::{
      __m128i,
      _mm_loadu_si128,
      _mm_shuffle_epi8,
      _mm_cmpeq_epi8,
      _mm_movemask_epi8,
    },
    mem,
    fmt,
  };

  pub const VECTOR_SIZE: usize = mem::size_of::<__m128i>();

  pub fn fmt_escaped_html(s: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let bytes = s.as_bytes();
    let mut mark = 0;

    unsafe {
      foreach_special_simd(bytes, 0, |i| {
        let c = *bytes.get_unchecked(i) as usize;
        let entry = super::ESCAPE_TABLE[c];
        f.write_str(s.get_unchecked(mark..i))?;
        mark = i + 1; // all escaped characters are ASCII
        if let Some(replacement) = entry {
          f.write_str(replacement)
        } else {
          f.write_str(s.get_unchecked(i..mark))
        } 
      })?;
      f.write_str(s.get_unchecked(mark..))
    }
  }

  unsafe fn foreach_special_simd<F: FnMut(usize) -> fmt::Result>(
    bytes: &[u8],
    mut offset: usize,
    mut callback: F,
  ) -> fmt::Result {
    // The strategy here is to walk the byte buffer in chunks of
    // VECTOR_SIZE (16) bytes at a time starting at the given offset.
    // For each chunk, we compute a a bitmask indicating whether the
    // corresponding byte is a HTML special byte. We then iterate over all
    // the 1 bits in this mask and call the callback function with the
    // corresponding index in the buffer.
    //
    // When the number of HTML special bytes in the buffer is relatively low,
    // this allows us to quickly go through the buffer without a lookup and
    // for every single byte.

    debug_assert!(bytes.len() >= VECTOR_SIZE);
    let upperbound = bytes.len() - VECTOR_SIZE;
    while offset < upperbound {
      let mut mask = compute_mask(bytes, offset);
      while mask != 0 {
        let ix = mask.trailing_zeros();
        callback(offset + ix as usize)?;
        mask ^= mask & -mask;
      }
      offset += VECTOR_SIZE;
    }

    // Final iteration. We align the read with the end of the slice and
    // shift off the bytes at start we have already scanned.
    let mut mask = compute_mask(bytes, upperbound);
    mask >>= offset - upperbound;
    while mask != 0 {
      let ix = mask.trailing_zeros();
      callback(offset + ix as usize)?;
      mask ^= mask & -mask;
    }
    Ok(())
  }

  #[target_feature(enable = "ssse3")]
  /// Computes a byte mask at given offset in the byte buffer. Its first 16
  /// (least significant) bits correspond to whether there is an HTML special
  /// byte (&, <, ", >) at the 16 bytes `bytes[offset..]`. For example, the
  /// mask `(1 << 3)` states that there is an HTML byte at `offset + 3`. It is
  /// only safe to call this function when `bytes.len() >= offset +
  /// VECTOR_SIZE`.
  unsafe fn compute_mask(bytes: &[u8], offset: usize) -> i32 {
    debug_assert!(bytes.len() >= offset + VECTOR_SIZE);

    let table = create_lookup();
    let lookup = _mm_loadu_si128(table.as_ptr() as *const __m128i);
    let raw_ptr = bytes.as_ptr().add(offset) as *const __m128i;

    // Load the vector from memory.
    let vector = _mm_loadu_si128(raw_ptr);

    // We take the least significant 4 bits of every byte and use them as
    // indices to map into the lookup vector.
    //
    // Note that shuffle maps bytes with their most significant bit set to
    // lookup[0]. Bytes that share their lower nibble with an HTML special
    // byte get mapped to that corresponding special byte. Note that all HTML
    // special bytes have distinct lower nibbles. Other bytes either get
    // mapped to 0 or 127.
    let expected = _mm_shuffle_epi8(lookup, vector);

    // We compare the original vector to the mapped output. Bytes that shared
    // a lower nibble with an HTML special byte match *only* if they are that
    // special byte. Bytes that have either a 0 lower nibble or their most
    // significant bit set were mapped to 127 and will hence never match. All
    // other bytes have non-zero lower nibbles but were mapped to 0 and will
    // therefore also not match.
    let matches = _mm_cmpeq_epi8(expected, vector);

    // Translate matches to a bitmask, where every 1 corresponds to a HTML
    // special character and a 0 is a non-HTML byte.
    _mm_movemask_epi8(matches)
  }

  /// Creates the lookup table for use in `compute_mask`.
  const fn create_lookup() -> [u8; 16] {
    let mut table = [0; 16];
    table[(b'<' & 0x0f)  as usize] = b'<';
    table[(b'>' & 0x0f)  as usize] = b'>';
    table[(b'&' & 0x0f)  as usize] = b'&';
    table[(b'"' & 0x0f)  as usize] = b'"';
    table[(b'\'' & 0x0f) as usize] = b'\'';
    table[0] = 0b0111_1111;
    table
  }
}
