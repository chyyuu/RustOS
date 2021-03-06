//! Syscalls for time

use super::*;
use crate::consts::USEC_PER_TICK;
use core::time::Duration;
use lazy_static::lazy_static;

/// should be initialized together
lazy_static! {
    pub static ref EPOCH_BASE: u64 = crate::arch::timer::read_epoch();
    pub static ref TICK_BASE: u64 = unsafe { crate::trap::TICK as u64 };
}

// 1ms msec
// 1us usec
// 1ns nsec

const USEC_PER_SEC: u64 = 1_000_000;
const MSEC_PER_SEC: u64 = 1_000;
const USEC_PER_MSEC: u64 = 1_000;
const NSEC_PER_USEC: u64 = 1_000;

/// Get time since epoch in usec
fn get_epoch_usec() -> u64 {
    let tick_base = *TICK_BASE;
    let epoch_base = *EPOCH_BASE;
    let tick = unsafe { crate::trap::TICK as u64 };

    (tick - tick_base) * USEC_PER_TICK as u64 + epoch_base * USEC_PER_SEC
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TimeVal {
    sec: u64,
    usec: u64,
}

impl TimeVal {
    pub fn to_msec(&self) -> u64 {
        self.sec * MSEC_PER_SEC + self.usec / USEC_PER_MSEC
    }

    pub fn get_epoch() -> Self {
        let usec = get_epoch_usec();
        TimeVal {
            sec: usec / USEC_PER_SEC,
            usec: usec % USEC_PER_SEC,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TimeSpec {
    sec: u64,
    nsec: u64,
}

impl TimeSpec {
    pub fn to_duration(&self) -> Duration {
        Duration::new(self.sec, self.nsec as u32)
    }

    pub fn get_epoch() -> Self {
        let usec = get_epoch_usec();
        TimeSpec {
            sec: usec / USEC_PER_SEC,
            nsec: usec % USEC_PER_SEC * NSEC_PER_USEC,
        }
    }
}

pub fn sys_gettimeofday(tv: *mut TimeVal, tz: *const u8) -> SysResult {
    info!("gettimeofday: tv: {:?}, tz: {:?}", tv, tz);
    if tz as usize != 0 {
        return Err(SysError::EINVAL);
    }

    let proc = process();
    proc.vm.check_write_ptr(tv)?;

    let timeval = TimeVal::get_epoch();
    unsafe {
        *tv = timeval;
    }
    Ok(0)
}

pub fn sys_clock_gettime(clock: usize, ts: *mut TimeSpec) -> SysResult {
    info!("clock_gettime: clock: {:?}, ts: {:?}", clock, ts);

    let proc = process();
    proc.vm.check_write_ptr(ts)?;

    let timespec = TimeSpec::get_epoch();
    unsafe {
        *ts = timespec;
    }
    Ok(0)
}

pub fn sys_time(time: *mut u64) -> SysResult {
    let sec = get_epoch_usec() / USEC_PER_SEC;
    if time as usize != 0 {
        let proc = process();
        proc.vm.check_write_ptr(time)?;
        unsafe {
            time.write(sec as u64);
        }
    }
    Ok(sec as usize)
}

// ignore other fields for now
#[repr(C)]
pub struct RUsage {
    utime: TimeVal,
    stime: TimeVal,
}

pub fn sys_getrusage(who: usize, rusage: *mut RUsage) -> SysResult {
    info!("getrusage: who: {}, rusage: {:?}", who, rusage);
    let proc = process();
    proc.vm.check_write_ptr(rusage)?;

    let tick_base = *TICK_BASE;
    let tick = unsafe { crate::trap::TICK as u64 };

    let usec = (tick - tick_base) * USEC_PER_TICK as u64;
    let new_rusage = RUsage {
        utime: TimeVal {
            sec: usec / USEC_PER_SEC,
            usec: usec % USEC_PER_SEC,
        },
        stime: TimeVal {
            sec: usec / USEC_PER_SEC,
            usec: usec % USEC_PER_SEC,
        },
    };
    unsafe { *rusage = new_rusage };
    Ok(0)
}
