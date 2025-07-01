use std::ffi::{CStr, CString};

pub fn cstr(str: &str) -> CString {
  CString::new(str).expect(format!("Could not create CString from {}", str).as_str())
}