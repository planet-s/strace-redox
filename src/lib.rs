// Needed because Redox is using an outdated Rust
#![feature(maybe_uninit)]

use bitflags::bitflags;
use std::{
    fmt,
    fs::File,
    io::{self, prelude::*, Result, SeekFrom},
    iter,
    mem::{self, MaybeUninit},
    ops::{Deref, DerefMut},
    os::unix::io::AsRawFd,
    ptr,
    slice
};

mod arch;
mod kernel;
mod f80;

fn e<T>(res: syscall::Result<T>) -> Result<T> {
    res.map_err(|err| io::Error::from_raw_os_error(err.errno))
}

bitflags! {
    pub struct Stop: u8 {
        const INSTRUCTION = syscall::PTRACE_SINGLESTEP;
        const SYSCALL = syscall::PTRACE_SYSCALL;
        const SYSEMU = syscall::PTRACE_SYSEMU;
        const COMPLETION = syscall::PTRACE_CONT;
    }
}

pub type Pid = usize;

#[derive(Clone, Copy, Debug)]
pub struct IntRegisters(pub syscall::IntRegisters);

impl IntRegisters {
    pub fn format_syscall_bare(&self) -> String {
        arch::format_syscall(None, &self)
    }
    pub fn format_syscall_full(&self, mem: &mut Memory) -> String {
        arch::format_syscall(Some(mem), &self)
    }
    pub fn return_value(&self) -> usize {
        arch::return_value(&self)
    }
}
impl Deref for IntRegisters {
    type Target = syscall::IntRegisters;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for IntRegisters {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FloatRegisters(pub syscall::FloatRegisters);

impl Deref for FloatRegisters {
    type Target = syscall::FloatRegisters;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for FloatRegisters {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[repr(u16)]
#[derive(Clone, Debug)]
pub enum PtraceEvent {
    Clone(Pid),
    Invalid(u16, [u8; mem::size_of::<syscall::PtraceEventContent>()])
}
impl PtraceEvent {
    pub fn new(inner: syscall::PtraceEvent) -> Self {
        unsafe {
            match inner.tag {
                syscall::PTRACE_EVENT_CLONE => PtraceEvent::Clone(inner.data.clone),
                _ => PtraceEvent::Invalid(inner.tag, mem::transmute(inner.data))
            }
        }
    }
}

pub struct Registers {
    pub float: File,
    pub int: File
}
impl Registers {
    pub fn attach(pid: Pid) -> Result<Self> {
        Ok(Self {
            float: File::open(format!("proc:{}/regs/float", pid))?,
            int: File::open(format!("proc:{}/regs/int", pid))?
        })
    }
    pub fn get_float(&mut self) -> Result<FloatRegisters> {
        let mut regs = syscall::FloatRegisters::default();
        self.float.read(&mut regs)?;
        Ok(FloatRegisters(regs))
    }
    pub fn set_float(&mut self, regs: &FloatRegisters) -> Result<()> {
        self.float.write(&regs)?;
        Ok(())
    }
    pub fn get_int(&mut self) -> Result<IntRegisters> {
        let mut regs = syscall::IntRegisters::default();
        self.int.read(&mut regs)?;
        Ok(IntRegisters(regs))
    }
    pub fn set_int(&mut self, regs: &IntRegisters) -> Result<()> {
        self.int.write(&regs)?;
        Ok(())
    }
}
impl fmt::Debug for Registers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Registers(...)")
    }
}

pub struct Memory {
    pub file: File
}
impl Memory {
    pub fn attach(pid: Pid) -> Result<Self> {
        Ok(Self {
            file: File::open(format!("proc:{}/mem", pid))?
        })
    }
    pub fn read(&mut self, from: *const u8, to: &mut [u8]) -> Result<()> {
        self.file.seek(SeekFrom::Start(from as u64))?;
        self.file.read(to)?;
        Ok(())
    }
    pub fn write(&mut self, from: &[u8], to: *const u8) -> Result<()> {
        self.file.seek(SeekFrom::Start(to as u64))?;
        self.file.write(from)?;
        Ok(())
    }
    pub fn cursor(&mut self) -> Result<u64> {
        self.file.seek(SeekFrom::Current(0))
    }
}
impl fmt::Debug for Memory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Memory(...)")
    }
}

pub struct Tracer {
    pub file: File,
    pub regs: Registers,
    pub mem: Memory
}
impl Tracer {
    pub fn attach(pid: Pid) -> Result<Self> {
        Ok(Self {
            file: File::open(format!("proc:{}/trace", pid))?,
            regs: Registers::attach(pid)?,
            mem: Memory::attach(pid)?
        })
    }
    /// Set a breakpoint on the next of stop, and wait for the
    /// breakpoint to be reached (unless tracer is
    /// nonblocking). Returns a reference to self to allow a tiny bit
    /// of chaining.
    pub fn next(&mut self, flags: Stop) -> Result<&mut Self> {
        if let Some(handler) = self.next_event(flags)? {
            handler.ignore()?;
        }
        Ok(self)
    }
    pub fn next_event(&mut self, flags: Stop) -> Result<Option<EventHandler>> {
        if self.file.write(&[flags.bits()])? != 0 {
            Ok(None)
        } else {
            Ok(Some(EventHandler { inner: self }))
        }
    }
    pub fn nonblocking(self) -> Result<NonblockTracer> {
        let old_flags = e(syscall::fcntl(self.file.as_raw_fd() as usize, syscall::F_GETFL, 0))?;
        let new_flags = old_flags | syscall::O_NONBLOCK;
        e(syscall::fcntl(self.file.as_raw_fd() as usize, syscall::F_SETFL, new_flags))?;
        Ok(NonblockTracer {
            old_flags,
            inner: self
        })
    }
}
impl fmt::Debug for Tracer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Tracer(...)")
    }
}

#[must_use = "The tracer may not be finished waiting unless you call `retry` here"]
pub struct EventHandler<'a> {
    inner: &'a mut Tracer
}
impl<'a> EventHandler<'a> {
    pub fn iter<'b>(&'b mut self) -> impl Iterator<Item = Result<PtraceEvent>> + 'b {
        let mut buf = [MaybeUninit::<syscall::PtraceEvent>::uninit(); 4];
        let mut i = 0;
        let mut len = 0;
        let trace = &mut *self.inner;
        iter::from_fn(move || {
            if i >= len {
                len = match trace.file.read(unsafe {
                    slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, buf.len() * mem::size_of::<syscall::PtraceEvent>())
                }) {
                    Ok(n) => n / mem::size_of::<syscall::PtraceEvent>(),
                    Err(err) => return Some(Err(err))
                };
                if len == 0 {
                    return None;
                }
                i = 0;
            }
            let ret = PtraceEvent::new(unsafe {
                ptr::read(buf[i].as_mut_ptr())
            });
            i += 1;
            Some(Ok(ret))
        })
    }
    /// Tries to wait for the initial breakpoint to be
    /// reached. Returns None if it managed, but itself if another
    /// event interrupted it and has to be handled.
    pub fn retry(self) -> Result<Option<Self>> {
        // Not using mem::forget in `else` block because of eventual
        // I/O errors that will drop
        if self.inner.file.write(&[syscall::PTRACE_WAIT])? == 0 {
            Ok(Some(self))
        } else {
            Ok(None)
        }
    }
    /// Handle events by calling a specified callback until breakpoint
    /// is reached
    pub fn from_callback<F, E>(mut self, mut callback: F) -> std::result::Result<(), E>
    where
        F: FnMut(PtraceEvent) -> std::result::Result<(), E>,
        E: From<io::Error>
    {
        loop {
            for event in self.iter() {
                callback(event?)?;
            }
            match self.retry()? {
                Some(me) => self = me,
                None => break
            }
        }
        Ok(())
    }
    /// Ignore events, just acknowledge them and move on
    pub fn ignore(self) -> Result<()> {
        self.from_callback(|_| Ok(()))
    }
}

pub struct NonblockTracer {
    old_flags: usize,
    inner: Tracer
}
impl NonblockTracer {
    pub fn blocking(self) -> Result<Tracer> {
        e(syscall::fcntl(self.file.as_raw_fd() as usize, syscall::F_SETFL, self.old_flags))?;
        Ok(self.inner)
    }
    pub fn wait(&mut self) -> Result<()> {
        self.file.write(&[syscall::PTRACE_WAIT])?;
        Ok(())
    }
}
impl Deref for NonblockTracer {
    type Target = Tracer;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl DerefMut for NonblockTracer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
impl fmt::Debug for NonblockTracer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NonblockTracer(...)")
    }
}
