use crate::{kernel::debug, Memory};
use std::io::Result;
use syscall::data::IntRegisters;

pub fn format_syscall(mem: Option<&mut Memory>, r: &IntRegisters) -> String {
    debug::format_call(mem, r.rax, r.rdi, r.rsi, r.rdx, r.r10, r.r8)
}
pub fn set_breakpoint(mem: &mut Memory, address: *const u8) -> Result<()> {
    mem.write(address, &[0xCC]) // int3
}
pub fn return_value(r: &IntRegisters) -> usize {
    r.rax
}
