//! HTML Escaping
//!
//! Stolen from pulldown-cmark-escape
//! <https://github.com/pulldown-cmark/pulldown-cmark/>

use std::fmt::{self, Display};

const ESCAPE_TABLE: [Option<&str>; 256] = create_escape_table();
const fn create_escape_table() -> [Option<&'static str>; 256] {
  let mut table = [None; 256];
  table[b'<'  as usize] = Some("&lt;");
  table[b'>'  as usize] = Some("&gt;");
  table[b'&'  as usize] = Some("&amp;");
  table[b'"'  as usize] = Some("&quot;");
  table[b'\'' as usize] = Some("&apos;");
  table
}

/// A wrapper for HTML-escaped strings
pub struct Escaped<'a>(pub &'a str);

// stolen from pulldown-cmark-escape
impl Display for Escaped<'_> {
  #[cfg(target_arch = "x86_64")]
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    // the SIMD accelerated code uses the PSHUFB instruction, which is part
    // of the SSSE3 instruction set
    if is_x86_feature_detected!("ssse3") {
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

// stolen from pulldown-cmark-escape
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

// stolen from pulldown-cmark-escape
#[cfg(target_arch = "x86_64")]
mod simd {
  use std::{arch::x86_64::*, mem, fmt};

  const VECTOR_SIZE: usize = mem::size_of::<__m128i>();

  #[inline]
  pub fn fmt_escaped_html(s: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    // the strategy here is to walk s in chunks of VECTOR_SIZE (16) bytes at
    // a time:
    //
    // 1. for each chunk, we compute a a bitmask indicating whether the
    //    corresponding byte is a HTML special byte
    // 2. for each bit set in this mask, we print the escaped character
    //    accordingly, as well as the surrounding characters that don't need
    //    escaping
    //
    // when the number of HTML special bytes in the buffer is relatively low,
    // this allows us to quickly go through the buffer without a lookup and
    // for every single byte
    if s.len() < VECTOR_SIZE {
      return super::fmt_escaped_html_scalar(s, f);
    }

    let bytes = s.as_bytes();
    let mut mark = 0;
    let mut offset = 0;

    unsafe {
      let upperbound = bytes.len() - VECTOR_SIZE;
      while offset < upperbound {
        let mut mask = compute_mask(bytes, offset);

        while mask != 0 {
          let first_special = mask.trailing_zeros();
          let i = offset + first_special as usize;
          let c = *bytes.get_unchecked(i) as usize;

          // here we know c = s[i] is a character that should be escaped,
          // so it is safe to unwrap ESCAPE_TABLE[c]
          let escape_seq = super::ESCAPE_TABLE[c].unwrap();
          f.write_str(s.get_unchecked(mark..i))?;
          f.write_str(escape_seq)?;

          mark = i + 1; // all escaped characters are ASCII
          mask ^= mask & -mask;
        }

        offset += VECTOR_SIZE;
      }

      // ======================================================================
      // final iteration: we align the read with the end of the slice
      // and shift off the bytes at start we have already scanned
      let mut mask = compute_mask(bytes, upperbound);
      mask >>= offset - upperbound;

      while mask != 0 {
        let first_special = mask.trailing_zeros();
        let i = offset + first_special as usize;
        let c = *bytes.get_unchecked(i) as usize;

        // here we know c = s[i] is a character that should be escaped,
        // so it is safe to unwrap ESCAPE_TABLE[c]
        let escape_seq = super::ESCAPE_TABLE[c].unwrap();
        f.write_str(s.get_unchecked(mark..i))?;
        f.write_str(escape_seq)?;

        mark = i + 1; // all escaped characters are ASCII
        mask ^= mask & -mask;
      }

      f.write_str(s.get_unchecked(mark..))
    }
  }


  #[inline]
  #[target_feature(enable = "ssse3")]
  /// Computes a byte mask at given offset in the byte buffer. Its first 16
  /// (least significant) bits correspond to whether there is an HTML special
  /// byte at the first VECTOR_SIZE bytes `bytes[offset..]`.
  ///
  /// It is only safe to call this function when `bytes.len() >= offset +
  /// VECTOR_SIZE`.
  unsafe fn compute_mask(bytes: &[u8], offset: usize) -> i32 {
    debug_assert!(bytes.len() >= offset + VECTOR_SIZE);

    const LOOKUP_TABLE: [u8; VECTOR_SIZE] = create_lookup();
    const fn create_lookup() -> [u8; VECTOR_SIZE] {
      let mut table = [0; VECTOR_SIZE];
      table[(b'<'  & 0x0f) as usize] = b'<';
      table[(b'>'  & 0x0f) as usize] = b'>';
      table[(b'&'  & 0x0f) as usize] = b'&';
      table[(b'"'  & 0x0f) as usize] = b'"';
      table[(b'\'' & 0x0f) as usize] = b'\'';
      table[0]                       = 0b01111111;
      table
    }

    let lookup_table = _mm_loadu_si128(
      LOOKUP_TABLE.as_ptr() as *const __m128i
    );
    let raw_ptr = bytes.as_ptr().add(offset) as *const __m128i;
    let vector = _mm_loadu_si128(raw_ptr);

    // mask the vector using the lookup table:
    //
    // 1. bytes whose lower nibbles are special HTML characters get mapped to
    //    their lower nibbles
    // 2. bytes whose lower nibbles are nonzero and *not* special HTML
    //    characters get mapped to 0
    // 3. bytes whose lower nibbles are 0 get mapped to 0b01111111
    let masked = _mm_shuffle_epi8(lookup_table, vector);

    // compare the original vector to the masked one:
    //
    // 1. bytes that shared a lower nibble with an HTML special byte match
    //    *only* if they are that special byte
    // 2. all other bytes will never match
    let matches = _mm_cmpeq_epi8(masked, vector);

    // translate matches to a bitmask: every 1 corresponds to a HTML
    // special character and a 0 is a non-HTML byte
    _mm_movemask_epi8(matches)
  }
}
