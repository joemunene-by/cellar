//! Headless wrapper around `unarc.dll`'s `FreeArcExtract` export.
//!
//! FitGirl repacks ship a 32-bit `unarc.dll` (FreeArc decompressor)
//! that ISDone.dll normally drives from inside the Inno Setup wizard.
//! On Apple Silicon Macs that wizard is unreachable: wine 11's
//! winemac.drv returns an HWND that FitGirl's `botva2.dll` cannot
//! subclass, and the install dies with Win32 error 1400 before any
//! file is written.
//!
//! This binary side-steps the wizard entirely. We load `unarc.dll`
//! ourselves, call `FreeArcExtract` with a progress callback, and
//! print the same kind of "filename / total / wrote" lines a CLI
//! decompressor would. No window, no subclass, no error 1400.
//!
//! Build (cross-compile):
//!   rustup target add i686-pc-windows-gnu
//!   apt install mingw-w64       # or: brew install mingw-w64
//!   cargo build --release --target i686-pc-windows-gnu
//!
//! Run (under wine, with `unarc.dll` and the `cls-*` plugins in the
//! same directory or on PATH):
//!   wine cellar-freearc.exe fg-01.bin C:\\Games\\CoD-MW3
//!
//! Stable exit codes:
//!   0   — extraction succeeded
//!   2   — bad CLI usage
//!   3   — could not load unarc.dll or find FreeArcExtract
//!   4   — FreeArcExtract returned a nonzero error code

use std::env;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::process::ExitCode;
use std::ptr;

use libloading::{Library, Symbol};

// FreeArcExtract's progress callback. The DLL invokes it many times
// per archive with one of these `what` tags:
//
//   "filename" — int1 is the about-to-extract file's size in bytes,
//                str is the destination filename (UTF-8).
//   "total"    — int1 is bytes processed, int2 is bytes total
//                (across the whole archive). Fires periodically.
//   "read"     — int1, int2: bytes read from the archive so far.
//   "write"    — int1, int2: bytes written to disk so far.
//   "error"    — str is the error message, int1 is the FreeArc
//                error code.
//
// Returning 0 continues; nonzero aborts the extraction.
type CallbackFn =
    unsafe extern "C" fn(what: *const c_char, int1: c_int, int2: c_int, str_param: *const c_char) -> c_int;

// FreeArcExtract is exported as cdecl variadic in the original
// FreeArc sources. On the caller side a fixed-arity declaration
// with the exact arg pattern we use (cmd + flags + "--" + archive +
// NULL terminator) is ABI-compatible because cdecl puts the burden
// of stack cleanup on the caller for both variadic and fixed.
//
// Args we pass:
//   cb       — our progress callback
//   "x"      — extract mode
//   "-y"     — assume yes on any prompt
//   "-o+"    — always overwrite existing files
//   dst      — "-dp" followed by the destination directory
//   "--"     — end of options sentinel
//   archive  — path to the .bin file (positional)
//   NULL     — terminator
type FreeArcExtractFn = unsafe extern "C" fn(
    cb: CallbackFn,
    cmd: *const c_char,
    opt_y: *const c_char,
    opt_overwrite: *const c_char,
    opt_dst: *const c_char,
    opt_sep: *const c_char,
    archive: *const c_char,
    terminator: *const c_char,
) -> c_int;

unsafe extern "C" fn progress_callback(
    what: *const c_char,
    int1: c_int,
    int2: c_int,
    str_param: *const c_char,
) -> c_int {
    // SAFETY: the DLL hands us C strings it owns for the duration of
    // the callback. We never store the pointer past return.
    let what_str = unsafe { c_str_to_string(what) };
    let extra_str = unsafe { c_str_to_string(str_param) };

    match what_str.as_str() {
        "filename" => println!("file {} ({} bytes)", extra_str, int1),
        "total" => {
            if int2 > 0 {
                let pct = (int1 as f64 / int2 as f64) * 100.0;
                println!("progress {}/{} ({:.1}%)", int1, int2, pct);
            }
        }
        // "read" and "write" fire a lot; only surface every-so-often
        // to keep stdout from being firehosed.
        "write" => {
            if int1 == int2 && int2 > 0 {
                println!("wrote {} bytes", int2);
            }
        }
        "error" => {
            eprintln!("freearc error {}: {}", int1, extra_str);
        }
        _ => {} // ignore "read" and any future tags
    }
    0
}

unsafe fn c_str_to_string(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: caller is the FreeArc DLL passing a NUL-terminated C string.
    unsafe { std::ffi::CStr::from_ptr(p) }
        .to_string_lossy()
        .into_owned()
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: cellar-freearc <archive.bin> <output-dir>");
        eprintln!("       both unarc.dll and any required cls-*.dll plugins must");
        eprintln!("       be in the same directory as this binary or on PATH.");
        return ExitCode::from(2);
    }

    let archive = PathBuf::from(&args[1]);
    let outdir = PathBuf::from(&args[2]);

    if !archive.exists() {
        eprintln!("archive not found: {}", archive.display());
        return ExitCode::from(2);
    }
    if let Err(err) = std::fs::create_dir_all(&outdir) {
        eprintln!("could not create output dir {}: {}", outdir.display(), err);
        return ExitCode::from(2);
    }

    // Load unarc.dll from the binary's own directory first, then PATH.
    // SAFETY: libloading::Library::new is unsafe because loading a
    // shared library can run arbitrary init code. We trust unarc.dll
    // (it's the standard FitGirl-shipped FreeArc decoder).
    let lib = match unsafe { Library::new("unarc.dll") } {
        Ok(l) => l,
        Err(err) => {
            eprintln!("could not load unarc.dll: {}", err);
            eprintln!("place unarc.dll next to this binary or on PATH.");
            return ExitCode::from(3);
        }
    };

    let extract: Symbol<FreeArcExtractFn> = match unsafe { lib.get(b"FreeArcExtract\0") } {
        Ok(s) => s,
        Err(err) => {
            eprintln!("could not find FreeArcExtract in unarc.dll: {}", err);
            return ExitCode::from(3);
        }
    };

    // Build the argv-like list of CStrings. Keep them owned for the
    // lifetime of the call so the C-side pointers stay valid.
    let c_cmd = CString::new("x").unwrap();
    let c_y = CString::new("-y").unwrap();
    let c_overwrite = CString::new("-o+").unwrap();
    let c_dst = CString::new(format!("-dp{}", outdir.display())).unwrap();
    let c_sep = CString::new("--").unwrap();
    let c_archive = CString::new(archive.display().to_string()).unwrap();

    println!("extract {} -> {}", archive.display(), outdir.display());

    // SAFETY: every pointer points into a CString we own and keep alive
    // until after the call returns; the function pointer signature matches
    // FreeArc's cdecl ABI.
    let rc = unsafe {
        extract(
            progress_callback,
            c_cmd.as_ptr(),
            c_y.as_ptr(),
            c_overwrite.as_ptr(),
            c_dst.as_ptr(),
            c_sep.as_ptr(),
            c_archive.as_ptr(),
            ptr::null(),
        )
    };

    if rc != 0 {
        eprintln!("FreeArcExtract returned {}", rc);
        return ExitCode::from(4);
    }
    println!("done");
    ExitCode::SUCCESS
}
