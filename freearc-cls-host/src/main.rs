//! Minimum-viable host for FreeArc CLS compression plugins.
//!
//! Loads one `cls-*.dll`, calls its single exported `ClsMain` entry
//! to decompress a stream, and prints the result on stdout. The
//! callback model (described in `Compression/_CLS/cls.h` in the
//! `xredor/unarc` source tree) means the plugin pulls bytes from us
//! via callbacks rather than via caller-allocated buffers.
//!
//! Usage from a wine host:
//!
//!   wine cellar-freearc-cls-host.exe \
//!        --dll C:\\path\\to\\cls-lolly.dll \
//!        --params d2g:al1 \
//!        < input.bin > output.bin
//!
//! Why bother: FitGirl repacks ship their data inside FreeArc
//! archives whose solid blocks use closed-source CLS plugins
//! (`lolzi`, `lolzx`, `lolly`, `lollypop`). cellar's native FreeArc
//! reader handles the open codecs (`storing`, `lzma`, `zstd`); for
//! the closed ones we shell out here. This sidesteps `unarc.dll`'s
//! console-mode probe state machine, which deadlocks on
//! wine-on-Mac (the IOCTL_CONDRV_GET_MODE call returns ACCESS_DENIED
//! and unarc waits forever on an event nothing signals). The CLS
//! plugins themselves never touch the console, so direct invocation
//! is safe.

use std::ffi::CString;
use std::io::{self, Read, Write};
use std::os::raw::{c_int, c_void};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use libloading::{Library, Symbol};

// ---------- CLS ABI constants (from Compression/_CLS/cls.h) ----------

const CLS_OK: c_int = 0;
const CLS_ERROR_NOT_IMPLEMENTED: c_int = -2;

// `operation` arg to ClsMain
const CLS_INIT: c_int = 1;
const CLS_DONE: c_int = 2;
#[allow(dead_code)]
const CLS_COMPRESS: c_int = 3;
const CLS_DECOMPRESS: c_int = 4;

// `callback_operation` arg the plugin uses in our callback
const CLS_MALLOC: c_int = 1;
const CLS_FREE: c_int = 2;
const CLS_GET_PARAMSTR: c_int = 3;
const CLS_SET_PARAMSTR: c_int = 4;
const CLS_THREADS: c_int = 5;
const CLS_MEMORY: c_int = 6;
const CLS_DECOMPRESSION_MEMORY: c_int = 7;
const CLS_DECOMPRESSOR_VERSION: c_int = 8;
const CLS_BLOCK: c_int = 9;
const CLS_EXPAND_DATA: c_int = 10;
const CLS_ID: c_int = 101;
const CLS_VERSION: c_int = 102;
const CLS_THREAD_SAFE: c_int = 103;
const CLS_FULL_READ: c_int = 4096;
const CLS_PARTIAL_READ: c_int = 5120;
const CLS_FULL_WRITE: c_int = 6144;
const CLS_PARTIAL_WRITE: c_int = 7168;

fn op_name(op: c_int) -> &'static str {
    match op {
        CLS_MALLOC => "MALLOC",
        CLS_FREE => "FREE",
        CLS_GET_PARAMSTR => "GET_PARAMSTR",
        CLS_SET_PARAMSTR => "SET_PARAMSTR",
        CLS_THREADS => "THREADS",
        CLS_MEMORY => "MEMORY",
        CLS_DECOMPRESSION_MEMORY => "DECOMPRESSION_MEMORY",
        CLS_DECOMPRESSOR_VERSION => "DECOMPRESSOR_VERSION",
        CLS_BLOCK => "BLOCK",
        CLS_EXPAND_DATA => "EXPAND_DATA",
        CLS_ID => "ID",
        CLS_VERSION => "VERSION",
        CLS_THREAD_SAFE => "THREAD_SAFE",
        CLS_FULL_READ => "FULL_READ",
        CLS_PARTIAL_READ => "PARTIAL_READ",
        CLS_FULL_WRITE => "FULL_WRITE",
        CLS_PARTIAL_WRITE => "PARTIAL_WRITE",
        _ => "<unknown>",
    }
}

type ClsCallback =
    unsafe extern "C" fn(instance: *mut c_void, op: c_int, ptr: *mut c_void, n: c_int) -> c_int;

type ClsMainFn = unsafe extern "C" fn(
    operation: c_int,
    callback: ClsCallback,
    instance: *mut c_void,
) -> c_int;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the cls-*.dll to load.
    #[arg(long)]
    dll: PathBuf,

    /// Plugin parameter string. This is the bit AFTER the codec name
    /// in a FreeArc method like "lolly:d2g:al1" (here, pass "d2g:al1").
    /// Empty is fine; some plugins have sensible defaults.
    #[arg(long, default_value = "")]
    params: String,

    /// Log every CLS callback the plugin makes to stderr, with the
    /// op name and arg. Use this to learn which ops a new plugin
    /// requires before extending the callback table.
    #[arg(long)]
    trace_callbacks: bool,
}

/// Held in the `instance` slot of every callback. The CLS plugin
/// never touches it; we use it for our per-call state.
struct CallbackCtx {
    // Pre-loaded compressed bytes (slurped from stdin).
    in_buf: Vec<u8>,
    in_pos: usize,
    // Accumulated decompressed output. Flushed to stdout after the
    // ClsMain call returns.
    out_buf: Vec<u8>,
    // C string passed back on CLS_GET_PARAMSTR queries. Owned so the
    // pointer stays valid across callbacks.
    params: CString,
    // If true, every callback dispatch is logged to stderr with its
    // op name + n value + return code.
    trace: bool,
}

/// CLS callback dispatch. Single function pointer the plugin calls
/// for every memory / IO / metadata request.
unsafe extern "C" fn cls_callback(
    instance: *mut c_void,
    op: c_int,
    ptr: *mut c_void,
    n: c_int,
) -> c_int {
    if instance.is_null() {
        return CLS_ERROR_NOT_IMPLEMENTED;
    }
    let ctx = unsafe { &mut *(instance as *mut CallbackCtx) };
    let rc = match op {
        CLS_FULL_READ | CLS_PARTIAL_READ => {
            // Plugin wants up to n bytes in *ptr. Return bytes read;
            // 0 means EOF.
            let want = if n < 0 { 0 } else { n as usize };
            let avail = ctx.in_buf.len().saturating_sub(ctx.in_pos);
            let take = avail.min(want);
            if take > 0 {
                let src = &ctx.in_buf[ctx.in_pos..ctx.in_pos + take];
                let dst = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, take) };
                dst.copy_from_slice(src);
                ctx.in_pos += take;
            }
            take as c_int
        }
        CLS_FULL_WRITE | CLS_PARTIAL_WRITE => {
            // Plugin has n bytes at *ptr for us to swallow.
            let len = if n < 0 { 0 } else { n as usize };
            let src = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) };
            ctx.out_buf.extend_from_slice(src);
            CLS_OK
        }
        CLS_GET_PARAMSTR => {
            // Caller-allocated buffer of size n at *ptr; we strncpy in.
            let cap = if n < 0 { 0 } else { n as usize };
            if cap == 0 {
                CLS_OK
            } else {
                let bytes = ctx.params.as_bytes_with_nul();
                let copy_len = bytes.len().min(cap);
                unsafe {
                    let dst = std::slice::from_raw_parts_mut(ptr as *mut u8, copy_len);
                    dst.copy_from_slice(&bytes[..copy_len]);
                    if copy_len == cap && cap > 0 && bytes.len() > cap {
                        *(ptr as *mut u8).add(cap - 1) = 0;
                    }
                }
                CLS_OK
            }
        }
        CLS_MALLOC => {
            // Plugin asks us to allocate n bytes and store the pointer
            // at *(void**)ptr. We leak the allocation; CLS_FREE below
            // is a no-op (process lives milliseconds).
            let bytes_wanted = if n <= 0 { 0 } else { n as usize };
            let v: Vec<u8> = vec![0u8; bytes_wanted];
            let boxed = v.into_boxed_slice();
            let raw = Box::leak(boxed).as_mut_ptr();
            unsafe {
                *(ptr as *mut *mut c_void) = raw as *mut c_void;
            }
            CLS_OK
        }
        CLS_FREE => CLS_OK,
        // Metadata queries the plugin makes about its environment.
        // Returning sensible defaults instead of NOT_IMPLEMENTED, in
        // case the plugin treats NOT_IMPLEMENTED as a hard failure.
        CLS_THREADS => 1, // single-threaded
        CLS_MEMORY | CLS_DECOMPRESSION_MEMORY => 0, // use plugin defaults
        CLS_THREAD_SAFE => 1, // we serialise all calls
        CLS_ID | CLS_VERSION | CLS_DECOMPRESSOR_VERSION => 0,
        CLS_BLOCK | CLS_EXPAND_DATA | CLS_SET_PARAMSTR => CLS_OK,
        _ => CLS_ERROR_NOT_IMPLEMENTED,
    };
    if ctx.trace {
        eprintln!(
            "[cls-trace] op={} (0x{:x}) n={} ptr={:p} rc={}",
            op_name(op),
            op,
            n,
            ptr,
            rc
        );
    }
    rc
}

fn main() -> ExitCode {
    let args = Args::parse();

    // Slurp stdin into memory. CLS is streaming on the plugin side
    // but having the input in a flat buffer keeps the host trivial
    // and is what FreeArc itself does for solid blocks anyway.
    let mut in_buf = Vec::new();
    if let Err(e) = io::stdin().lock().read_to_end(&mut in_buf) {
        eprintln!("read stdin: {}", e);
        return ExitCode::from(2);
    }

    let params = match CString::new(args.params.clone()) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("--params contains an interior NUL byte");
            return ExitCode::from(3);
        }
    };

    // CELLAR_CLS_TRACE env wins over --trace-callbacks so the
    // tracing can be enabled end-to-end (env propagates from the
    // Rust caller through wine into us) without touching the
    // dispatch code in freearc-native.
    let trace = args.trace_callbacks || std::env::var("CELLAR_CLS_TRACE").is_ok();
    if trace {
        eprintln!(
            "[cls-trace] host start: dll={} params={:?}",
            args.dll.display(),
            args.params
        );
    }

    let mut ctx = Box::new(CallbackCtx {
        in_buf,
        in_pos: 0,
        out_buf: Vec::new(),
        params,
        trace,
    });

    // SAFETY: libloading::Library + ClsMain ABI. We hold the Library
    // until after the last ClsMain call. We leak both the Library
    // and the CallbackCtx at the end so any DLL-internal pointers
    // stay valid through process tear-down (Windows runs DLL
    // _DLL_PROCESS_DETACH on exit, and the leak is harmless there).
    let rc = unsafe {
        let lib = Library::new(&args.dll).unwrap_or_else(|e| {
            eprintln!("LoadLibrary({}): {}", args.dll.display(), e);
            std::process::exit(4);
        });
        let cls_main: Symbol<ClsMainFn> = lib.get(b"ClsMain\0").unwrap_or_else(|e| {
            eprintln!("GetProcAddress(ClsMain): {}", e);
            std::process::exit(5);
        });

        let ctx_ptr = ctx.as_mut() as *mut CallbackCtx as *mut c_void;
        if trace {
            eprintln!("[cls-trace] ClsMain(CLS_INIT)");
        }
        let init_rc = cls_main(CLS_INIT, cls_callback, ctx_ptr);
        if trace {
            eprintln!("[cls-trace] ClsMain(CLS_INIT) returned {}", init_rc);
            eprintln!("[cls-trace] ClsMain(CLS_DECOMPRESS) — input {} bytes", ctx.in_buf.len());
        }
        let rc = cls_main(CLS_DECOMPRESS, cls_callback, ctx_ptr);
        if trace {
            eprintln!("[cls-trace] ClsMain(CLS_DECOMPRESS) returned {}", rc);
            eprintln!("[cls-trace] ClsMain(CLS_DONE)");
        }
        let _ = cls_main(CLS_DONE, cls_callback, ctx_ptr);

        std::mem::forget(lib);
        rc
    };

    if rc != CLS_OK {
        eprintln!("ClsMain(CLS_DECOMPRESS) returned {}", rc);
        return ExitCode::from(rc.abs().min(127) as u8);
    }

    if let Err(e) = io::stdout().lock().write_all(&ctx.out_buf) {
        eprintln!("write stdout: {}", e);
        return ExitCode::from(6);
    }

    std::mem::forget(ctx);
    ExitCode::SUCCESS
}
