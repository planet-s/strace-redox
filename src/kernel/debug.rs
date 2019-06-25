//! Function to print a syscall, lifted from the kernel's
//! src/syscall/debug.rs

use std::{ascii, mem};

use syscall::{
    data::{Map, Stat, TimeSpec},
    flag::*,
    number::*
};

/// TODO: When being able to read child's memory
unsafe fn raw_slice<T>(_ptr: *const T, _len: usize) -> &'static [T] {
    std::slice::from_raw_parts("<memory goes here>".as_ptr() as *const T, 18)
}

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

pub unsafe fn format_call(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> String {
    match a {
        SYS_OPEN => format!(
            "open({:?}, {:#X})",
            ByteStr(raw_slice(b as *const u8, c)),
            d
        ),
        SYS_CHMOD => format!(
            "chmod({:?}, {:#o})",
            ByteStr(raw_slice(b as *const u8, c)),
            d
        ),
        SYS_RMDIR => format!(
            "rmdir({:?})",
            ByteStr(raw_slice(b as *const u8, c))
        ),
        SYS_UNLINK => format!(
            "unlink({:?})",
            ByteStr(raw_slice(b as *const u8, c))
        ),
        SYS_CLOSE => format!(
            "close({})", b
        ),
        SYS_DUP => format!(
            "dup({}, {:?})",
            b,
            ByteStr(raw_slice(c as *const u8, d))
        ),
        SYS_DUP2 => format!(
            "dup2({}, {}, {:?})",
            b,
            c,
            ByteStr(raw_slice(d as *const u8, e))
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
            raw_slice(
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
            raw_slice(
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
            ByteStr(raw_slice(b as *const u8, c))
        ),
        SYS_CLOCK_GETTIME => format!(
            "clock_gettime({}, {:?})",
            b,
            raw_slice(c as *const TimeSpec, 1)
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
            raw_slice(c as *const [usize; 2], d).iter()
                .map(|a| std::str::from_utf8(raw_slice(a[0] as *const u8, a[1])).ok())
                .collect::<Vec<Option<&str>>>(),
            raw_slice(e as *const [usize; 2], f).iter()
                .map(|a| std::str::from_utf8(raw_slice(a[0] as *const u8, a[1])).ok())
                .collect::<Vec<Option<&str>>>()
        ),
        SYS_FUTEX => format!(
            "futex({:#X} [{:?}], {}, {}, {}, {})",
            b,
            raw_slice(b as *const i32, 1)[0],
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
            raw_slice(c as *const [u64; 2], 1),
            raw_slice(d as *const [u64; 2], 1)
        ),
        SYS_MKNS => format!(
            "mkns({:?})",
            raw_slice(b as *const [usize; 2], c)
        ),
        SYS_MPROTECT => format!(
            "mprotect({:#X}, {}, {:#X})",
            b,
            c,
            d
        ),
        SYS_NANOSLEEP => format!(
            "nanosleep({:?}, ({}, {}))",
            raw_slice(b as *const TimeSpec, 1),
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
            raw_slice(b as *const usize, 2),
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
