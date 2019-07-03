//! Function to print a syscall, lifted from the kernel's
//! src/syscall/debug.rs

use std::{any::TypeId, ascii, mem};

use crate::Memory;
use syscall::{
    data::{Map, Stat, TimeSpec},
    flag::*,
    number::*
};

struct ByteStr<'a>(&'a[u8]);

impl<'a> ::core::fmt::Debug for ByteStr<'a> {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        write!(f, "\"")?;
        for i in self.0 {
            for ch in ascii::escape_default(*i) {
                write!(f, "{}", ch as char)?;
            }
        }
        write!(f, "\"")?;
        Ok(())
    }
}

fn raw_slice<T: Copy + 'static>(mem: Option<&mut Memory>, ptr: *const T, len: usize) -> Vec<T> {
    let mut buf = vec![0; len * mem::size_of::<T>()];

    if mem.and_then(|mem| mem.read(ptr as *const u8, &mut buf).ok()).is_none() {
        const ERROR_STRING: &[u8] = b"error";

        if TypeId::of::<T>() == TypeId::of::<u8>() && buf.len() >= ERROR_STRING.len() {
            buf[..ERROR_STRING.len()].copy_from_slice(ERROR_STRING);
        }
    }

    // FIXME: The capacity after shrink_to_fit may still be more than
    // the length... Memory leak?
    buf.shrink_to_fit();

    let raw = buf.as_mut_ptr() as *mut T;
    let cap = buf.capacity() / mem::size_of::<T>();

    mem::forget(buf);

    unsafe {
        Vec::from_raw_parts(raw, len, cap)
    }
}

pub fn format_call(mut mem: Option<&mut Memory>, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> String {
    macro_rules! raw_slice {
        ($ptr:expr, $len:expr) => {
            &raw_slice!(owned $ptr, $len)
        };
        (owned $ptr:expr, $len:expr) => {
            raw_slice(match mem {
                Some(ref mut mem) => Some(&mut *mem),
                None => None
            }, $ptr, $len)
        };
    }
    match a {
        SYS_OPEN => format!(
            "open({:?}, {:#X})",
            ByteStr(raw_slice!(b as *const u8, c)),
            d
        ),
        SYS_CHMOD => format!(
            "chmod({:?}, {:#o})",
            ByteStr(raw_slice!(b as *const u8, c)),
            d
        ),
        SYS_RMDIR => format!(
            "rmdir({:?})",
            ByteStr(raw_slice!(b as *const u8, c))
        ),
        SYS_UNLINK => format!(
            "unlink({:?})",
            ByteStr(raw_slice!(b as *const u8, c))
        ),
        SYS_CLOSE => format!(
            "close({})", b
        ),
        SYS_DUP => format!(
            "dup({}, {:?})",
            b,
            ByteStr(raw_slice!(c as *const u8, d))
        ),
        SYS_DUP2 => format!(
            "dup2({}, {}, {:?})",
            b,
            c,
            ByteStr(raw_slice!(d as *const u8, e))
        ),
        SYS_READ => format!(
            "read({}, {:#X}, {})",
            b,
            c,
            d
        ),
        SYS_WRITE => format!(
            "write({}, {:#X}, {})",
            b,
            c,
            d
        ),
        SYS_LSEEK => format!(
            "lseek({}, {}, {} ({}))",
            b,
            c as isize,
            match d {
                SEEK_SET => "SEEK_SET",
                SEEK_CUR => "SEEK_CUR",
                SEEK_END => "SEEK_END",
                _ => "UNKNOWN"
            },
            d
        ),
        SYS_FCNTL => format!(
            "fcntl({}, {} ({}), {:#X})",
            b,
            match c {
                F_DUPFD => "F_DUPFD",
                F_GETFD => "F_GETFD",
                F_SETFD => "F_SETFD",
                F_SETFL => "F_SETFL",
                F_GETFL => "F_GETFL",
                _ => "UNKNOWN"
            },
            c,
            d
        ),
        SYS_FMAP => format!(
            "fmap({}, {:?})",
            b,
            raw_slice!(
                c as *const Map,
                d/mem::size_of::<Map>()
            ),
        ),
        SYS_FUNMAP => format!(
            "funmap({:#X})",
            b
        ),
        SYS_FPATH => format!(
            "fpath({}, {:#X}, {})",
            b,
            c,
            d
        ),
        SYS_FSTAT => format!(
            "fstat({}, {:?})",
            b,
            raw_slice!(
                c as *const Stat,
                d/mem::size_of::<Stat>()
            ),
        ),
        SYS_FSTATVFS => format!(
            "fstatvfs({}, {:#X}, {})",
            b,
            c,
            d
        ),
        SYS_FSYNC => format!(
            "fsync({})",
            b
        ),
        SYS_FTRUNCATE => format!(
            "ftruncate({}, {})",
            b,
            c
        ),

        SYS_BRK => format!(
            "brk({:#X})",
            b
        ),
        SYS_CHDIR => format!(
            "chdir({:?})",
            ByteStr(raw_slice!(b as *const u8, c))
        ),
        SYS_CLOCK_GETTIME => format!(
            "clock_gettime({}, {:?})",
            b,
            raw_slice!(c as *const TimeSpec, 1)
        ),
        SYS_CLONE => format!(
            "clone({})",
            b
        ),
        SYS_EXIT => format!(
            "exit({})",
            b
        ),
        //TODO: Cleanup, do not allocate
        SYS_FEXEC => format!(
            "fexec({}, {:?}, {:?})",
            b,
            raw_slice!(c as *const [usize; 2], d).iter()
                .map(|a| String::from_utf8(raw_slice!(owned a[0] as *const u8, a[1])).ok())
                .collect::<Vec<Option<String>>>(),
            raw_slice!(e as *const [usize; 2], f).iter()
                .map(|a| String::from_utf8(raw_slice!(owned a[0] as *const u8, a[1])).ok())
                .collect::<Vec<Option<String>>>()
        ),
        SYS_FUTEX => format!(
            "futex({:#X} [{:?}], {}, {}, {}, {})",
            b,
            raw_slice!(b as *const i32, 1)[0],
            c,
            d,
            e,
            f
        ),
        SYS_GETCWD => format!(
            "getcwd({:#X}, {})",
            b,
            c
        ),
        SYS_GETEGID => format!("getegid()"),
        SYS_GETENS => format!("getens()"),
        SYS_GETEUID => format!("geteuid()"),
        SYS_GETGID => format!("getgid()"),
        SYS_GETNS => format!("getns()"),
        SYS_GETPID => format!("getpid()"),
        SYS_GETUID => format!("getuid()"),
        SYS_IOPL => format!(
            "iopl({})",
            b
        ),
        SYS_KILL => format!(
            "kill({}, {})",
            b,
            c
        ),
        SYS_SIGRETURN => format!("sigreturn()"),
        SYS_SIGACTION => format!(
            "sigaction({}, {:#X}, {:#X}, {:#X})",
            b,
            c,
            d,
            e
        ),
        SYS_SIGPROCMASK => format!(
            "sigprocmask({}, {:?}, {:?})",
            b,
            raw_slice!(c as *const [u64; 2], 1),
            raw_slice!(d as *const [u64; 2], 1)
        ),
        SYS_MKNS => format!(
            "mkns({:?})",
            raw_slice!(b as *const [usize; 2], c)
        ),
        SYS_MPROTECT => format!(
            "mprotect({:#X}, {}, {:#X})",
            b,
            c,
            d
        ),
        SYS_NANOSLEEP => format!(
            "nanosleep({:?}, ({}, {}))",
            raw_slice!(b as *const TimeSpec, 1),
            c,
            d
        ),
        SYS_PHYSALLOC => format!(
            "physalloc({})",
            b
        ),
        SYS_PHYSFREE => format!(
            "physfree({:#X}, {})",
            b,
            c
        ),
        SYS_PHYSMAP => format!(
            "physmap({:#X}, {}, {:#X})",
            b,
            c,
            d
        ),
        SYS_PHYSUNMAP => format!(
            "physunmap({:#X})",
            b
        ),
        SYS_VIRTTOPHYS => format!(
            "virttophys({:#X})",
            b
        ),
        SYS_PIPE2 => format!(
            "pipe2({:?}, {})",
            raw_slice!(b as *const usize, 2),
            c
        ),
        SYS_SETREGID => format!(
            "setregid({}, {})",
            b,
            c
        ),
        SYS_SETRENS => format!(
            "setrens({}, {})",
            b,
            c
        ),
        SYS_SETREUID => format!(
            "setreuid({}, {})",
            b,
            c
        ),
        SYS_UMASK => format!(
            "umask({:#o}",
            b
        ),
        SYS_WAITPID => format!(
            "waitpid({}, {:#X}, {})",
            b,
            c,
            d
        ),
        SYS_YIELD => format!("yield()"),
        _ => format!(
            "UNKNOWN{} {:#X}({:#X}, {:#X}, {:#X}, {:#X}, {:#X})",
            a, a,
            b,
            c,
            d,
            e,
            f
        )
    }
}
