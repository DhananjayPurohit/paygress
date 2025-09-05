// Simple NGINX Plugin - Compiles to .so
// cargo build --release --features nginx-plugin

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

// Simple function that NGINX can call
#[no_mangle]
pub extern "C" fn paygress_verify_payment(token: *const c_char, amount: c_int) -> c_int {
    if token.is_null() {
        return 1; // Error: no token
    }

    unsafe {
        let token_str = match CStr::from_ptr(token).to_str() {
            Ok(s) => s,
            Err(_) => return 1, // Error: invalid token
        };

        // Simple verification logic
        if token_str.is_empty() {
            return 1; // Error: empty token
        }

        // For demo: accept any token that contains the amount
        if token_str.contains(&amount.to_string()) {
            0 // Success: payment verified
        } else {
            1 // Error: payment failed
        }
    }
}

// Simple pod provisioning function
#[no_mangle]
pub extern "C" fn paygress_provision_pod(namespace: *const c_char, pod_name: *const c_char) -> c_int {
    if namespace.is_null() || pod_name.is_null() {
        return 1; // Error
    }

    unsafe {
        let ns = match CStr::from_ptr(namespace).to_str() {
            Ok(s) => s,
            Err(_) => return 1,
        };

        let name = match CStr::from_ptr(pod_name).to_str() {
            Ok(s) => s,
            Err(_) => return 1,
        };

        // For demo: just return success
        // In real implementation, call Kubernetes API here
        println!("Provisioning pod: {}/{}", ns, name);
        0 // Success
    }
}

// Get version info
#[no_mangle]
pub extern "C" fn paygress_version() -> *const c_char {
    b"paygress-1.0.0\0".as_ptr() as *const c_char
}
