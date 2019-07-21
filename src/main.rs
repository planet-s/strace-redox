use std::{
    env,
    ffi::OsString,
    io::{Error, Result},
    mem,
    os::unix::ffi::OsStrExt,
    path::PathBuf,
};

use strace::{EventHandler, Pid, Tracer, Stop};

fn e<T>(res: syscall::Result<T>) -> Result<T> {
    res.map_err(|err| Error::from_raw_os_error(err.errno))
}

fn main() -> Result<()> {
    let cmd = match env::args().nth(1) {
        Some(cmd) => cmd,
        None => {
            eprintln!("Usage: strace <path>");
            return Ok(());
        }
    };

    let mut file = None;
    for mut path in env::split_paths(&env::var_os("PATH").unwrap_or(OsString::new())) {
        path.push(&cmd);
        if let Ok(fd) = e(syscall::open(&path.as_os_str().as_bytes(), syscall::O_RDONLY)) {
            file = Some((path, fd));
            break;
        }
    }

    let (path, fd) = match file {
        Some(inner) => inner,
        None => {
            eprintln!("Could not find that binary in $PATH");
            return Ok(());
        }
    };

    match e(unsafe { syscall::clone(0) })? {
        0 => child(fd),
        pid => parent(path, pid)
    }
}

fn child(fd: usize) -> Result<()> {
    let mut args = Vec::new();
    for arg in env::args().skip(1) {
        let len = arg.len();
        let ptr = arg.as_ptr() as usize;
        mem::forget(arg);
        args.push([ptr, len]);
    }

    let mut vars = Vec::new();
    for (key, val) in env::vars() {
        let combined = format!("{}={}", key, val);

        let len = combined.len();
        let ptr = combined.as_ptr() as usize;
        mem::forget(combined);
        vars.push([ptr, len]);
    }

    // I'm ready to be traced!
    e(syscall::kill(e(syscall::getpid())?, syscall::SIGSTOP))?;

    e(syscall::fexec(fd, &args, &vars))?;
    unreachable!("fexec can't return Ok(_)")
}

fn parent(path: PathBuf, pid: Pid) -> Result<()> {
    let mut status = 0;

    eprintln!("Executing {} (PID {})", path.display(), pid);

    // Wait until child is ready to be traced
    e(syscall::waitpid(pid, &mut status, syscall::WUNTRACED))?;

    let mut tracer = Tracer::attach(pid)?;

    // Won't actually restart the process, because it's stopped by ptrace
    e(syscall::kill(pid, syscall::SIGCONT))?;

    let mut main_loop = move || -> Result<()> {
        let handle = |handler: Option<EventHandler>| if let Some(handler) = handler {
            handler.from_callback(|event| -> Result<()> {
                eprintln!("EVENT: {:?}", event);
                Ok(())
            })
        } else {
            Ok(())
        };
        loop {
            handle(tracer.next_event(Stop::SYSCALL)?)?;
            let regs = tracer.regs.get_int()?;
            let syscall = regs.format_syscall_full(&mut tracer.mem);
            eprintln!("SYSCALL:     {}", syscall);

            if regs.0.rax == syscall::SYS_FEXEC {
                // fexec(...) doesn't return on success
                continue;
            }

            handle(tracer.next_event(Stop::SYSCALL)?)?;
            let regs = tracer.regs.get_int()?;
            let ret = regs.return_value();

            eprint!("SYSCALL RET: {} = ", syscall);
            match syscall::Error::demux(ret) {
                Ok(val) => eprintln!("Ok({} ({:#X}))", val, val),
                Err(err) => eprintln!("Err(\"{}\" ({:#X})) ({:#X})", err, err.errno, ret),
            }
        }
    };
    match main_loop() {
        Err(ref err) if err.raw_os_error() == Some(syscall::ESRCH) => {
            e(syscall::waitpid(pid, &mut status, syscall::WNOHANG))?;
            if syscall::wifexited(status) {
                println!("Process exited with status {}", syscall::wexitstatus(status));
            }
            if syscall::wifsignaled(status) {
                println!("Process signaled with status {}", syscall::wtermsig(status));
            }
            Ok(())
        },
        other => other
    }
}
