use std::ffi::{CStr, CString};

pub fn cstr(str: &str) -> CString {
  CString::new(str).expect(format!("Could not create CString from {}", str).as_str())
}

pub fn split_bits(n: u32) -> Vec<u32> {
  (0..32).rev()
    .map(|i| {
      let mask = 1 << i;
      let matches = n & mask == mask;
      if matches {
          1
      } else {
          0
      }
    })
    .collect()
}