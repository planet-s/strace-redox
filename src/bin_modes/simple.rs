use std::{env, io::Result, process};

use strace::{Flags, Pid, Tracer};

pub struct Opt {
    pub cmd: Vec<String>,
}

pub fn parse_args() -> Opt {
    let cmd: Vec<_> = env::args().skip(1).collect();
    if cmd.is_empty() {
        eprintln!("Usage: strace <path>");
        process::exit(1);
    }
    Opt { cmd }
}

pub fn inner_main(_pid: Pid, mut tracer: Tracer, _opt: Opt) -> Result<()> {
    let mut unclosed = Vec::new();

    loop {
        let event =
            tracer
                .next_event(crate::TRACE_FLAGS)?
                .from_callback(|event| -> Result<()> {
                    eprintln!("EVENT: {:?}", event);
                    Ok(())
                })?;

        if event.cause == Flags::STOP_PRE_SYSCALL {
            let regs = tracer.regs.get_int()?;
            let syscall = regs.format_syscall_full(&mut tracer.mem);

            eprintln!("SYSCALL:     {}", syscall);
            unclosed.push(syscall);
        } else if event.cause == Flags::STOP_POST_SYSCALL {
            let syscall = unclosed.pop();
            let syscall = syscall
                .as_ref()
                .map(|s| &**s)
                .unwrap_or("<unmatched syscall>");

            let regs = tracer.regs.get_int()?;
            let ret = regs.return_value();

            eprint!("SYSCALL RET: {} = ", syscall);
            match syscall::Error::demux(ret) {
                Ok(val) => eprintln!("Ok({} ({:#X}))", val, val),
                Err(err) => eprintln!("Err(\"{}\" ({:#X})) ({:#X})", err, err.errno, ret),
            }
        } else {
            eprintln!("OTHER EVENT: {:?}", event);
        }
    }
}
