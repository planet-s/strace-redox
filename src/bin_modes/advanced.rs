use std::{
    collections::HashMap,
    fs::File,
    io::{prelude::*, Result},
    os::unix::io::AsRawFd,
};
use syscall::{data::Event, flag::EVENT_READ};

use strace::{EventData, Flags, NonblockTracer, Pid, Tracer};

use structopt::StructOpt;

#[derive(StructOpt)]
// Only make `pub` features that are in both simple and advanced modes
pub struct Opt {
    #[structopt(short, long)]
    /// Specify whether or not strace should trace more than just the
    /// top level child process
    recursive: bool,
    /// Specify the command and arguments to run
    pub cmd: Vec<String>,
}

pub fn parse_args() -> Opt {
    Opt::from_args()
}

struct Handle {
    pid: Pid,
    tracer: NonblockTracer,
    unclosed: Vec<String>,
}

pub fn inner_main(root: Pid, tracer: Tracer, opt: Opt) -> Result<()> {
    let mut tracer = tracer.nonblocking()?;
    tracer.next(crate::TRACE_FLAGS)?;

    let mut events = File::open("event:")?;

    let mut next_id = 0;
    events.write(&Event {
        id: tracer.file.as_raw_fd() as usize,
        flags: EVENT_READ,
        data: next_id,
    })?;

    let mut tracers = HashMap::new();
    tracers.insert(
        next_id,
        Handle {
            pid: root,
            tracer,
            unclosed: Vec::new(),
        },
    );
    next_id += 1;

    loop {
        let mut event = Event::default();
        events.read(&mut event)?;
        let index = event.data;

        let handle = tracers.get_mut(&index).unwrap();
        handle.tracer.next(crate::TRACE_FLAGS)?;

        for event in handle.tracer.events()? {
            let event = event?;

            // We don't want to mutably borrow tracer across the
            // entire loop - rather, re-fetch it at each iteration.
            let handle = tracers.get_mut(&index).unwrap();

            if event.cause == Flags::STOP_PRE_SYSCALL {
                let regs = handle.tracer.regs.get_int()?;
                let syscall = regs.format_syscall_full(&mut handle.tracer.mem);

                eprintln!("SYSCALL     (pid {}): {}", handle.pid, syscall);
                handle.unclosed.push(syscall);
            } else if event.cause == Flags::STOP_POST_SYSCALL {
                let syscall = handle.unclosed.pop();
                let syscall = syscall
                    .as_ref()
                    .map(|s| &**s)
                    .unwrap_or("<unmatched syscall>");

                let regs = handle.tracer.regs.get_int()?;
                let ret = regs.return_value();

                eprint!("SYSCALL RET (pid {}): {} = ", handle.pid, syscall);
                match syscall::Error::demux(ret) {
                    Ok(val) => eprintln!("Ok({} ({:#X}))", val, val),
                    Err(err) => eprintln!("Err(\"{}\" ({:#X})) ({:#X})", err, err.errno, ret),
                }
            } else {
                eprintln!("OTHER EVENT: {:?}", event);

                if opt.recursive {
                    if let EventData::EventClone(pid) = event.data {
                        let mut child = NonblockTracer::attach(pid)?;
                        child.next(crate::TRACE_FLAGS)?;

                        events.write(&Event {
                            id: child.file.as_raw_fd() as usize,
                            flags: EVENT_READ,
                            data: next_id,
                        })?;

                        tracers.insert(
                            next_id,
                            Handle {
                                pid,
                                tracer: child,
                                unclosed: Vec::new(),
                            },
                        );
                        next_id += 1;
                    }
                }
            }
        }
    }
}
