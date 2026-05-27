//! Headless wrapper around `unarc.dll`'s `FreeArcExtract` export.
//!
//! See README.md for the why.

use std::env;
use std::ffi::CString;
use std::mem;
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::process::ExitCode;

use libloading::{Library, Symbol};

// Callback FreeArc invokes during extraction. The DLL was compiled
// with __cdecl in upstream FreeArc.h, so we match that. Tags we have
// observed in the wild:
//
//   "filename" — int1 is the about-to-extract file's size in bytes,
//                str is the destination filename.
//   "total"    — int1 / int2: bytes done / bytes total over the
//                whole archive. Fires periodically.
//   "read"     — int1 / int2: bytes read from the archive so far.
//   "write"    — int1 / int2: bytes written to disk so far.
//   "error"    — str is the error message, int1 is the FreeArc error
//                code.
//
// Return 0 to continue, nonzero to abort.
type CallbackFn = unsafe extern "C" fn(
    what: *const c_char,
    int1: c_int,
    int2: c_int,
    str_param: *const c_char,
) -> c_int;

// Inno Setup's FitGirl scripts declare FreeArcExtract as a fixed
// 4-arg cdecl: (callback, cmd, archive, output_dir). Most other
// documented Pascal wrappers use the same shape. The original C
// header is variadic, but FreeArc parses only this many args in
// practice; passing more reads garbage from the next stack slot
// and the function returns -1 without ever calling the callback.
type FreeArcExtractFn = unsafe extern "C" fn(
    cb: CallbackFn,
    cmd: *const c_char,
    archive: *const c_char,
    output_dir: *const c_char,
) -> c_int;

unsafe extern "C" fn progress_callback(
    what: *const c_char,
    int1: c_int,
    int2: c_int,
    str_param: *const c_char,
) -> c_int {
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
        "write" => {
            if int1 == int2 && int2 > 0 {
                println!("wrote {} bytes", int2);
            }
        }
        "error" => {
            eprintln!("freearc error {}: {}", int1, extra_str);
        }
        other => eprintln!("[cb {}] int1={} int2={} str={:?}", other, int1, int2, extra_str),
    }
    0
}

unsafe fn c_str_to_string(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { std::ffi::CStr::from_ptr(p) }
        .to_string_lossy()
        .into_owned()
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: cellar-freearc <archive.bin> <output-dir>");
        return ExitCode::from(2);
    }

    let archive = PathBuf::from(&args[1]);
    let outdir = PathBuf::from(&args[2]);

    if let Err(err) = std::fs::create_dir_all(&outdir) {
        eprintln!("could not create output dir {}: {}", outdir.display(), err);
        return ExitCode::from(2);
    }

    let lib = match unsafe { Library::new("unarc.dll") } {
        Ok(l) => l,
        Err(err) => {
            eprintln!("could not load unarc.dll: {}", err);
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

    let c_cmd = CString::new("x").unwrap();
    let c_archive = CString::new(archive.display().to_string()).unwrap();
    let c_outdir = CString::new(outdir.display().to_string()).unwrap();

    println!("extract {} -> {}", archive.display(), outdir.display());

    // Detach from the console RIGHT BEFORE we hand control to unarc.dll.
    // Wine 11.8 on macOS returns ACCESS_DENIED for IOCTL_CONDRV_GET_MODE
    // on the default console attached to a console-subsystem PE.
    // unarc.dll misreads that as "interactive console" and spins up a
    // progress UI thread that deadlocks (call chain ends in
    // select(timeout=infinite) on an event that never signals).
    // With FreeConsole called, GetConsoleMode returns
    // ERROR_INVALID_HANDLE, which unarc handles by skipping its
    // progress UI and going straight to file-mode output through our
    // callback. No-op on real Windows when no console is inherited.
    //
    // Flushing stdout first so our setup prints make it out before we
    // sever the console handle. Status prints AFTER this point go to
    // an opened log file instead.
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    unsafe extern "system" {
        fn FreeConsole() -> i32;
    }
    let _ = unsafe { FreeConsole() };

    let log_path = std::env::var_os("CELLAR_FREEARC_LOG")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("cellar-freearc.log"));
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok();

    if let Some(ref mut f) = log {
        let _ = writeln!(f, "cellar-freearc starting extract -> {}", outdir.display());
    }

    let rc = unsafe {
        extract(
            progress_callback,
            c_cmd.as_ptr(),
            c_archive.as_ptr(),
            c_outdir.as_ptr(),
        )
    };

    if let Some(ref mut f) = log {
        let _ = writeln!(f, "FreeArcExtract returned {}", rc);
    }
    eprintln!("FreeArcExtract returned {}", rc);
    eprintln!("(log file: {})", log_path.display());

    // Leak the library on purpose. unarc.dll's DllMain DETACH path
    // page-faults when wine tears it down after main(); avoiding the
    // FreeLibrary call lets the process exit cleanly. The OS will
    // reclaim the mapping at process exit anyway.
    mem::forget(extract);
    mem::forget(lib);

    if rc != 0 {
        return ExitCode::from(4);
    }
    ExitCode::SUCCESS
}
