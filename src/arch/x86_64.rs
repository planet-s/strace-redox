use crate::{f80::f80_to_f64, kernel::debug, FloatRegisters, Memory};
use syscall::data::IntRegisters;

pub fn format_syscall(mem: Option<&mut Memory>, r: &IntRegisters) -> String {
    debug::format_call(mem, r.rax, r.rdi, r.rsi, r.rdx, r.r10, r.r8)
}
pub fn return_value(r: &IntRegisters) -> usize {
    r.rax
}

impl FloatRegisters {
    pub fn st_space_nth(&self, nth: usize) -> f64 {
        f80_to_f64(self.0.st_space[nth])
    }
    pub fn st_space(&self) -> [f64; 8] {
        let mut out = [0.0; 8];
        for (i, &n) in { self.0.st_space }.iter().enumerate() {
            out[i] = f80_to_f64(n);
        }
        out
    }
}
