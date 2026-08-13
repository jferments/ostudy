//! Android-only external transfer routing.
//!
//! The actual platform I/O stays in `platform::android`; this module exists so
//! Android can use the streaming transfer contract without duplicating the
//! desktop command symbols.

#![cfg(target_os = "android")]
