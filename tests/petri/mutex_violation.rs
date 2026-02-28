//@compile-flags: -Zmiri-disable-stacked-borrows -Zmiri-petri=tests/petri/petri_config.json
//@ignore-windows
//@ignore-android
//
// Minimal test for the Petri net monitor: two threads using a Mutex correctly.
// Run with: MIRIFLAGS="-Zmiri-petri=tests/petri/petri_config.json" cargo miri test tests/petri/mutex_violation.rs
//
// To test a violation, one would need a program that e.g. double-unlocks or
// unlocks without holding - such programs typically require unsafe code.

#![no_main]

use std::sync::Mutex;

static M: Mutex<i32> = Mutex::new(0);

#[unsafe(no_mangle)]
fn miri_start(_argc: isize, _argv: *const *const u8) -> isize {
    let handle = std::thread::spawn(|| {
        let mut g = M.lock().unwrap();
        *g += 1;
    });
    {
        let mut g = M.lock().unwrap();
        *g += 1;
    }
    handle.join().unwrap();
    0
}
