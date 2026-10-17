// Temporary developer sampler (never committed): SIGPROF frame-pointer walk.
// Writes raw return addresses plus the image slide to $LRR_SAMPLE_OUT.
use std::sync::atomic::{AtomicUsize, Ordering};

const MAX_SAMPLES: usize = 200_000;
const DEPTH: usize = 96;
static mut BUF: [u64; MAX_SAMPLES * DEPTH] = [0; MAX_SAMPLES * DEPTH];
static NEXT: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
struct SigAction {
    handler: usize,
    mask: u32,
    flags: i32,
}
#[repr(C)]
struct TimeVal {
    sec: i64,
    usec: i32,
    _pad: i32,
}
#[repr(C)]
struct ITimerVal {
    interval: TimeVal,
    value: TimeVal,
}
unsafe extern "C" {
    fn sigaction(sig: i32, act: *const SigAction, old: *mut SigAction) -> i32;
    fn setitimer(which: i32, new: *const ITimerVal, old: *mut ITimerVal) -> i32;
    fn pthread_self() -> usize;
    fn pthread_get_stackaddr_np(thread: usize) -> usize;
    fn pthread_get_stacksize_np(thread: usize) -> usize;
    fn _dyld_get_image_vmaddr_slide(index: u32) -> isize;
}

extern "C" fn handler(_sig: i32, _info: usize, context: usize) {
    unsafe {
        let slot = NEXT.fetch_add(1, Ordering::Relaxed);
        if slot >= MAX_SAMPLES || context == 0 {
            return;
        }
        let mcontext = *((context + 48) as *const usize);
        if mcontext == 0 {
            return;
        }
        let fp0 = *((mcontext + 16 + 29 * 8) as *const u64);
        let lr = *((mcontext + 16 + 30 * 8) as *const u64);
        let pc = *((mcontext + 16 + 32 * 8) as *const u64);
        let thread = pthread_self();
        let top = pthread_get_stackaddr_np(thread) as u64;
        let bottom = top - pthread_get_stacksize_np(thread) as u64;
        let base = slot * DEPTH;
        let buf = &raw mut BUF;
        let mut n = 0;
        (*buf)[base + n] = pc;
        n += 1;
        (*buf)[base + n] = lr;
        n += 1;
        let mut fp = fp0;
        while n < DEPTH - 1 && fp >= bottom && fp + 16 <= top && fp % 8 == 0 {
            let next = *(fp as *const u64);
            let ret = *((fp + 8) as *const u64);
            if ret == 0 {
                break;
            }
            (*buf)[base + n] = ret;
            n += 1;
            if next <= fp {
                break;
            }
            fp = next;
        }
        (*buf)[base + n] = 0;
    }
}

fn start() {
    unsafe {
        let action = SigAction {
            handler: handler as usize,
            mask: 0,
            flags: 0x40 | 0x2,
        };
        sigaction(27, &action, std::ptr::null_mut());
        let timer = ITimerVal {
            interval: TimeVal { sec: 0, usec: 2000, _pad: 0 },
            value: TimeVal { sec: 0, usec: 2000, _pad: 0 },
        };
        setitimer(2, &timer, std::ptr::null_mut());
    }
}

fn finish() {
    unsafe {
        let timer = ITimerVal {
            interval: TimeVal { sec: 0, usec: 0, _pad: 0 },
            value: TimeVal { sec: 0, usec: 0, _pad: 0 },
        };
        setitimer(2, &timer, std::ptr::null_mut());
        let Some(path) = std::env::var_os("LRR_SAMPLE_OUT") else {
            return;
        };
        let count = NEXT.load(Ordering::Relaxed).min(MAX_SAMPLES);
        let mut out = String::new();
        out.push_str(&format!("slide {:x}\n", _dyld_get_image_vmaddr_slide(0)));
        let buf = &raw const BUF;
        for slot in 0..count {
            let base = slot * DEPTH;
            let mut line = Vec::new();
            for i in 0..DEPTH {
                let value = (*buf)[base + i];
                if value == 0 {
                    break;
                }
                line.push(format!("{value:x}"));
            }
            out.push_str(&line.join(" "));
            out.push('\n');
        }
        let _ = std::fs::write(path, out);
    }
}

fn main() {
    start();
    let code = lrr::cli::entry("lrr");
    finish();
    std::process::exit(code);
}
