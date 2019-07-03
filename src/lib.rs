use std::{
    fs::File,
    io::{prelude::*, Result, SeekFrom}
};

mod arch;
mod kernel;

pub type Pid = usize;

#[derive(Clone, Copy)]
pub struct IntRegisters(pub syscall::IntRegisters);

impl IntRegisters {
    pub fn format_syscall_bare(&self) -> String {
        arch::format_syscall(None, &self.0)
    }
    pub fn format_syscall_full(&self, mem: &mut Memory) -> String {
        arch::format_syscall(Some(mem), &self.0)
    }
    pub fn return_value(&self) -> usize {
        arch::return_value(&self.0)
    }
}

pub struct Registers {
    int: File
}
impl Registers {
    pub fn attach(pid: Pid) -> Result<Self> {
        Ok(Self {
            int: File::open(format!("proc:{}/regs/int", pid))?
        })
    }
    pub fn get_int(&mut self) -> Result<IntRegisters> {
        let mut regs = syscall::IntRegisters::default();
        self.int.read(&mut regs)?;
        Ok(IntRegisters(regs))
    }
    pub fn set_int(&mut self, regs: &IntRegisters) -> Result<()> {
        self.int.write(&regs.0)?;
        Ok(())
    }
}

pub struct Memory {
    file: File
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
}

pub struct Tracer {
    file: File,
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
    pub fn next_syscall(&mut self) -> Result<()> {
        self.file.write(&[syscall::PTRACE_SYSCALL])?;
        Ok(())
    }
}
