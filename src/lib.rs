// Needed because Redox is using an outdated Rust
#![feature(maybe_uninit)]

use bitflags::bitflags;
use std::{
    fmt,
    fs::{File, OpenOptions},
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
    pub struct Flags: u64 {
        const STOP_PRE_SYSCALL = syscall::PTRACE_STOP_PRE_SYSCALL;
        const STOP_POST_SYSCALL = syscall::PTRACE_STOP_POST_SYSCALL;
        const STOP_SINGLESTEP = syscall::PTRACE_STOP_SINGLESTEP;
        const STOP_SIGNAL = syscall::PTRACE_STOP_SIGNAL;
        const STOP_ALL = Self::STOP_PRE_SYSCALL.bits | Self::STOP_POST_SYSCALL.bits | Self::STOP_SINGLESTEP.bits | Self::STOP_SIGNAL.bits;

        const EVENT_CLONE = syscall::PTRACE_EVENT_CLONE;
        const EVENT_ALL = Self::EVENT_CLONE.bits;

        const FLAG_SYSEMU = syscall::PTRACE_FLAG_SYSEMU;
        const FLAG_WAIT = syscall::PTRACE_FLAG_WAIT;
        const FLAG_ALL = Self::FLAG_SYSEMU.bits | Self::FLAG_WAIT.bits;
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EventData {
    EventClone(usize),
    StopSignal(usize, usize),
    Unknown(usize, usize, usize, usize, usize, usize),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Event {
    pub cause: Flags,
    pub data: EventData
}
impl Event {
    pub fn new(inner: syscall::PtraceEvent) -> Self {
        Self {
            cause: Flags::from_bits_truncate(inner.cause),
            data: match inner.cause {
                syscall::PTRACE_EVENT_CLONE => EventData::EventClone(inner.a),
                syscall::PTRACE_STOP_SIGNAL => EventData::StopSignal(inner.a, inner.b),
                _ => EventData::Unknown(inner.a, inner.b, inner.c, inner.d, inner.e, inner.f),
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
        self.file.read_exact(to)?;
        Ok(())
    }
    pub fn write(&mut self, from: &[u8], to: *const u8) -> Result<()> {
        self.file.seek(SeekFrom::Start(to as u64))?;
        self.file.write_all(from)?;
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
    /// Attach to a tracer with the specified PID. This will stop it.
    pub fn attach(pid: Pid) -> Result<Self> {
        Ok(Self {
            file: OpenOptions::new().read(true).write(true).truncate(true).open(format!("proc:{}/trace", pid))?,
            regs: Registers::attach(pid)?,
            mem: Memory::attach(pid)?
        })
    }
    /// Set a breakpoint on the next specified stop, and wait for the
    /// breakpoint to be reached. For convenience in the majority of
    /// use-cases, this panics on non-breakpoint events and returns
    /// the breaking event whenever the first matching breakpoint is
    /// hit. For being able to use non-breakpoint events, see the
    /// `next_event` function.
    pub fn next(&mut self, flags: Flags) -> Result<Event> {
        self.next_event(flags)?.from_callback(
            |event| panic!("`Tracer::next` should never be used to \
            handle non-breakpoint events, see `Tracer::next_event` \
            instead. Event: {:?}", event)
        )
    }
    /// Similarly to `next`, but instead of conveniently returning a
    /// breakpoint event, it returns an event handler that lets you
    /// handle events yourself.
    pub fn next_event(&mut self, flags: Flags) -> Result<EventHandler> {
        self.file.write(&flags.bits().to_ne_bytes())?;
        Ok(EventHandler { inner: self })
    }
    /// Convert this tracer to be nonblocking. Setting breakpoints
    /// will no longer wait by default, but you will gain access to a
    /// `wait` function which will do the same as in blocking
    /// mode. Useful for multiplexing tracers using the `event:`
    /// scheme.
    pub fn nonblocking(self) -> Result<NonblockTracer> {
        let old_flags = e(syscall::fcntl(self.file.as_raw_fd() as usize, syscall::F_GETFL, 0))?;
        let new_flags = old_flags | syscall::O_NONBLOCK;
        e(syscall::fcntl(self.file.as_raw_fd() as usize, syscall::F_SETFL, new_flags))?;
        Ok(NonblockTracer {
            old_flags,
            inner: self
        })
    }
    /// Same as `EventHandler::iter`, but does not rely on having an
    /// event handler. When only using a blocking tracer you shouldn't
    /// need to worry about this.
    pub fn events(&'_ mut self) -> impl Iterator<Item = Result<Event>> + '_ {
        let mut buf = [MaybeUninit::<syscall::PtraceEvent>::uninit(); 4];
        let mut i = 0;
        let mut len = 0;
        iter::from_fn(move || {
            if i >= len {
                len = match self.file.read(unsafe {
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
            let ret = Event::new(unsafe {
                ptr::read(buf[i].as_mut_ptr())
            });
            i += 1;
            Some(Ok(ret))
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
    /// Pop one event. Prefer the use of the `iter` function instead
    /// as it batches reads. Only reason for this would be to have
    /// control over exactly what gets requested from to the kernel.
    pub fn pop_one(&mut self) -> Result<Option<Event>> {
        let mut event = syscall::PtraceEvent::default();
        match self.inner.file.read(&mut event)? {
            0 => Ok(None),
            _ => Ok(Some(Event::new(event)))
        }
    }
    /// Returns an iterator over ptrace events. This iterator is not a
    /// fused one: It does NOT guarantee that reaching the end means
    /// it'll never get another one. This is because non-breakpoint
    /// events keep the tracee still running which can add more to the
    /// queue at any time essentially.
    pub fn iter(&'_ mut self) -> impl Iterator<Item = Result<Event>> + '_ {
        self.inner.events()
    }
    /// Tries to wait for a breakpoint event to be reached. To find
    /// out if a breakpoint event *was* reached, use the `iter`
    /// function to get events.
    pub fn retry(&mut self) -> Result<()> {
        self.inner.file.write(&Flags::FLAG_WAIT.bits().to_ne_bytes())?;
        Ok(())
    }
    /// Handle events by calling a specified callback until breakpoint
    /// is reached
    pub fn from_callback<F, E>(mut self, mut callback: F) -> std::result::Result<Event, E>
    where
        F: FnMut(Event) -> std::result::Result<(), E>,
        E: From<io::Error>
    {
        'outer: loop {
            let mut events = self.iter();

            while let Some(event) = events.next() {
                let event = event?;

                if event.cause & Flags::EVENT_ALL == event.cause {
                    callback(event)?;
                } else {
                    let next = events.next();
                    assert!(
                        next.is_none(),
                        "Breakpoint event wasn't final event. This is possible to handle, but usually not what you want."
                    );
                    break 'outer Ok(event);
                }
            }

            drop(events);

            self.retry()?;
        }
    }
    /// Ignore events, just acknowledge them and move on
    pub fn ignore(self) -> Result<Event> {
        self.from_callback(|_| Ok(()))
    }
}

pub struct NonblockTracer {
    old_flags: usize,
    inner: Tracer
}
impl NonblockTracer {
    /// Sets a breakpoint on the specified stop, without doing
    /// anything else: No handling of events, no getting what
    /// breakpoint actually caused this, no waiting for the
    /// breakpoint.
    pub fn next(&mut self, flags: Flags) -> Result<()> {
        self.file.write(&flags.bits().to_ne_bytes())?;
        Ok(())
    }
    /// Stub that prevents you from accidentally calling `next_event`
    /// on the tracer, do not use.
    #[deprecated(since = "forever", note = "Do not use next_event on a nonblocking tracer")]
    pub fn next_event(&mut self, _flags: Flags) -> Result<EventHandler> {
        panic!("Tried to use next_event on a nonblocking tracer")
    }

    /// Convert this tracer back to a blocking version. Any yet unread
    /// events are ignored.
    pub fn blocking(mut self) -> Result<Tracer> {
        self.events().for_each(|_| ());
        e(syscall::fcntl(self.file.as_raw_fd() as usize, syscall::F_SETFL, self.old_flags))?;
        Ok(self.inner)
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
