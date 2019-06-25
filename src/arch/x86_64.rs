use crate::kernel::debug;
use syscall::data::IntRegisters;

pub unsafe fn format_syscall(r: &IntRegisters) -> String {
    debug::format_call(r.rax, r.rdi, r.rsi, r.rdx, r.r10, r.r8)
}
pub fn return_value(r: &IntRegisters) -> usize {
    r.rax
}
