use std::{
    env,
    ffi::OsString,
    io::{Error, Result},
    mem,
    os::unix::ffi::OsStrExt
};

use strace::{Pid, Tracer};

fn e<T>(res: syscall::Result<T>) -> Result<T> {
    res.map_err(|err| Error::from_raw_os_error(err.errno))
}

fn main() -> Result<()> {
    match e(unsafe { syscall::clone(0) })? {
        0 => child(),
        pid => parent(pid)
    }
}

fn child() -> Result<()> {
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
            println!("{}", path.display());
            file = Some(fd);
            break;
        }
    }

    let file = match file {
        Some(file) => file,
        None => {
            eprintln!("Could not find that binary in $PATH");
            return Ok(());
        }
    };

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

    e(syscall::fexec(file, &args, &vars))?;
    unreachable!("fexec can't return Ok(_)")
}

fn parent(pid: Pid) -> Result<()> {
    let mut status = 0;

    // Wait until child is ready to be traced
    e(syscall::waitpid(pid, &mut status, syscall::WUNTRACED))?;

    let mut tracer = Tracer::attach(pid)?;

    // Won't actually restart the process, because it's stopped by ptrace
    e(syscall::kill(pid, syscall::SIGCONT))?;

    let mut main_loop = move || -> Result<()> {
        loop {
            tracer.next_syscall()?;
            let regs = tracer.regs.get_int()?;
            let syscall = regs.format_syscall_full(&mut tracer.mem);
            eprintln!("SYSCALL:     {}", syscall);

            if regs.0.rax == syscall::SYS_FEXEC {
                // fexec(...) doesn't return on success
                continue;
            }

            tracer.next_syscall()?;
            let regs = tracer.regs.get_int()?;
            let ret = regs.return_value();
            eprintln!("SYSCALL RET: {} = {} ({:#X})", syscall, ret, ret);
        }
    };
    match main_loop() {
        Err(ref err) if err.raw_os_error() == Some(syscall::ESRCH) => {
            e(syscall::waitpid(pid, &mut status, syscall::WNOHANG))?;
            if syscall::wifexited(status) {
                println!("Process exited with status {}", syscall::wexitstatus(status));
            }
            Ok(())
        },
        other => other
    }
}
