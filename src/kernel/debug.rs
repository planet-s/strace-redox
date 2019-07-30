//! Function to print a syscall, lifted from the kernel's
//! src/syscall/debug.rs

use std::{
    ascii,
    io::{Error, ErrorKind, Result},
    mem::{self, MaybeUninit},
    slice
};

use crate::Memory;
use syscall::{
    data::{Map, Stat, TimeSpec},
    flag::*,
    number::*
};

struct ByteStr(Vec<u8>);

impl ::core::fmt::Debug for ByteStr {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        write!(f, "\"")?;
        for i in &self.0 {
            for ch in ascii::escape_default(*i) {
                write!(f, "{}", ch as char)?;
            }
        }
        write!(f, "\"")?;
        Ok(())
    }
}

fn validate_slice<T: Copy + 'static>(mem: Option<&mut Memory>, ptr: *const T, len: usize) -> Result<Vec<T>> {
    let mut buf = vec![MaybeUninit::<T>::uninit(); len];

    {
        // Read raw bytes
        let mut byte_buf = unsafe {
            slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, len * mem::size_of::<T>())
        };

        let mem = mem.ok_or(Error::new(ErrorKind::InvalidData, "No memory inputted"))?;
        mem.read(ptr as *const u8, &mut byte_buf)?;
    }

    // Reinterpret Vec<MaybeUninit<T>> as Vec<T>
    let (ptr, len, cap) = (buf.as_mut_ptr(), buf.len(), buf.capacity());
    let ptr = ptr as *mut T;
    unsafe {
        mem::forget(buf);
        Ok(Vec::<T>::from_raw_parts(ptr, len, cap))
    }
}

pub fn format_call(mut mem: Option<&mut Memory>, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> String {
    macro_rules! validate_slice {
        ($ptr:expr, $len:expr) => {
            validate_slice(match mem {
                Some(ref mut mem) => Some(&mut *mem),
                None => None
            }, $ptr, $len)
        };
    }
    // The below code should preferrably only have the minimal amount
    // of changes diverging from src/syscall/debug.rs. At any point it
    // should not be too much effort to reset this to that code, and
    // patch any *absolutely* necessary changes.
    //
    // Things that need fixing:
    // - s/validate_slice\(_mut\)?(/validate_slice!(/g
    // - SYS_FEXEC str::from_utf8 -> String::from_utf8
    // - generally, any references -> owned values
    match a {
        SYS_OPEN => format!(
            "open({:?}, {:#X})",
            validate_slice!(b as *const u8, c).map(ByteStr),
            d
        ),
        SYS_CHMOD => format!(
            "chmod({:?}, {:#o})",
            validate_slice!(b as *const u8, c).map(ByteStr),
            d
        ),
        SYS_RMDIR => format!(
            "rmdir({:?})",
            validate_slice!(b as *const u8, c).map(ByteStr)
        ),
        SYS_UNLINK => format!(
            "unlink({:?})",
            validate_slice!(b as *const u8, c).map(ByteStr)
        ),
        SYS_CLOSE => format!(
            "close({})", b
        ),
        SYS_DUP => format!(
            "dup({}, {:?})",
            b,
            validate_slice!(c as *const u8, d).map(ByteStr)
        ),
        SYS_DUP2 => format!(
            "dup2({}, {}, {:?})",
            b,
            c,
            validate_slice!(d as *const u8, e).map(ByteStr)
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
        SYS_FCHMOD => format!(
            "fchmod({}, {:#o})",
            b,
            c
        ),
        SYS_FCHOWN => format!(
            "fchown({}, {}, {})",
            b,
            c,
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
            validate_slice!(
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
        SYS_FRENAME => format!(
            "frename({}, {:?})",
            b,
            validate_slice!(c as *const u8, d).map(ByteStr),
        ),
        SYS_FSTAT => format!(
            "fstat({}, {:?})",
            b,
            validate_slice!(
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
        SYS_FUTIMENS => format!(
            "futimens({}, {:?})",
            b,
            validate_slice!(
                c as *const TimeSpec,
                d/mem::size_of::<TimeSpec>()
            ),
        ),

        SYS_BRK => format!(
            "brk({:#X})",
            b
        ),
        SYS_CHDIR => format!(
            "chdir({:?})",
            validate_slice!(b as *const u8, c).map(ByteStr)
        ),
        SYS_CLOCK_GETTIME => format!(
            "clock_gettime({}, {:?})",
            b,
            validate_slice!(c as *mut TimeSpec, 1)
        ),
        SYS_CLONE => format!(
            "clone({:?})",
            CloneFlags::from_bits(b)
        ),
        SYS_EXIT => format!(
            "exit({})",
            b
        ),
        //TODO: Cleanup, do not allocate
        SYS_FEXEC => format!(
            "fexec({}, {:?}, {:?})",
            b,
            validate_slice!(
                c as *const [usize; 2],
                d
            ).map(|slice| {
                slice.iter().map(|a|
                    validate_slice!(a[0] as *const u8, a[1]).ok()
                    .and_then(|s| String::from_utf8(s).ok())
                ).collect::<Vec<Option<String>>>()
            }),
            validate_slice!(
                e as *const [usize; 2],
                f
            ).map(|slice| {
                slice.iter().map(|a|
                    validate_slice!(a[0] as *const u8, a[1]).ok()
                    .and_then(|s| String::from_utf8(s).ok())
                ).collect::<Vec<Option<String>>>()
            })
        ),
        SYS_FUTEX => format!(
            "futex({:#X} [{:?}], {}, {}, {}, {})",
            b,
            validate_slice!(b as *mut i32, 1).map(|uaddr| uaddr[0]),
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
        SYS_GETPGID => format!("getpgid()"),
        SYS_GETPID => format!("getpid()"),
        SYS_GETPPID => format!("getppid()"),
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
            validate_slice!(c as *const [u64; 2], 1),
            validate_slice!(d as *const [u64; 2], 1)
        ),
        SYS_MKNS => format!(
            "mkns({:?})",
            validate_slice!(b as *const [usize; 2], c)
        ),
        SYS_MPROTECT => format!(
            "mprotect({:#X}, {}, {:?})",
            b,
            c,
            MapFlags::from_bits(d)
        ),
        SYS_NANOSLEEP => format!(
            "nanosleep({:?}, ({}, {}))",
            validate_slice!(b as *const TimeSpec, 1),
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
            "physmap({:#X}, {}, {:?})",
            b,
            c,
            PhysmapFlags::from_bits(d)
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
            validate_slice!(b as *mut usize, 2),
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
            "waitpid({}, {:#X}, {:?})",
            b,
            c,
            WaitFlags::from_bits(d)
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
