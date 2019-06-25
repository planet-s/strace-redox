use std::{
    fs::File,
    io::{prelude::*, Result}
};
use syscall::data::IntRegisters;

mod arch;
mod kernel;

pub type Pid = usize;

#[derive(Clone, Copy)]
pub struct Registers(pub IntRegisters);

impl Registers {
    pub unsafe fn format_syscall(&self) -> String {
        arch::format_syscall(&self.0)
    }
    pub fn return_value(&self) -> usize {
        arch::return_value(&self.0)
    }
}

pub struct Tracer {
    trace_file: File,
    regs_file: File
}
impl Tracer {
    pub fn attach(pid: Pid) -> Result<Self> {
        Ok(Self {
            trace_file: File::open(format!("proc:{}/trace", pid))?,
            regs_file: File::open(format!("proc:{}/regs/int", pid))?
        })
    }
    pub fn next_syscall(&mut self) -> Result<()> {
        self.trace_file.write(&[syscall::PTRACE_SYSCALL])?;
        Ok(())
    }
    pub fn getregs(&mut self) -> Result<Registers> {
        let mut regs = IntRegisters::default();
        self.regs_file.read(&mut regs)?;
        Ok(Registers(regs))
    }
    pub fn setregs(&mut self, regs: &Registers) -> Result<()> {
        self.regs_file.write(&regs.0)?;
        Ok(())
    }
}
