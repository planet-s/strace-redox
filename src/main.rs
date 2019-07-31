use std::{
    env,
    ffi::OsString,
    io::{Error, Result},
    mem,
    os::unix::ffi::OsStrExt,
    path::PathBuf,
};

use strace::{Flags, Pid, Tracer};

mod bin_modes;

use bin_modes as mode;

fn e<T>(res: syscall::Result<T>) -> Result<T> {
    res.map_err(|err| Error::from_raw_os_error(err.errno))
}

pub const TRACE_FLAGS: Flags = Flags::from_bits_truncate(
    (Flags::STOP_ALL.bits() & !Flags::STOP_SINGLESTEP.bits()) | Flags::EVENT_ALL.bits(),
);

fn main() -> Result<()> {
    let opt = mode::parse_args();

    let mut file = None;
    for mut path in env::split_paths(&env::var_os("PATH").unwrap_or(OsString::new())) {
        path.push(&opt.cmd[0]);
        if let Ok(fd) = e(syscall::open(
            &path.as_os_str().as_bytes(),
            syscall::O_RDONLY,
        )) {
            file = Some((path, fd));
            break;
        }
    }

    let (path, fd) = match file {
        Some(inner) => inner,
        None => {
            eprintln!("Could not find that binary in $PATH");
            return Ok(());
        },
    };

    match e(unsafe { syscall::clone(syscall::CloneFlags::empty()) })? {
        0 => child(fd, opt.cmd.clone()),
        pid => parent(path, pid, opt),
    }
}

fn child(fd: usize, cmd_args: Vec<String>) -> Result<()> {
    let mut args = Vec::new();
    for arg in cmd_args {
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

fn parent(path: PathBuf, pid: Pid, opt: mode::Opt) -> Result<()> {
    let mut status = 0;

    eprintln!("Executing {} (PID {})", path.display(), pid);

    // Wait until child is ready to be traced
    e(syscall::waitpid(pid, &mut status, syscall::WUNTRACED))?;

    let mut tracer = Tracer::attach(pid)?;

    // Won't actually restart the process, because it's stopped by ptrace
    e(syscall::kill(pid, syscall::SIGCONT))?;

    // There will first be a post-syscall for `kill`.
    tracer.next(Flags::STOP_POST_SYSCALL)?;

    match mode::inner_main(pid, tracer, opt) {
        Err(ref err) if err.raw_os_error() == Some(syscall::ESRCH) => {
            e(syscall::waitpid(pid, &mut status, syscall::WNOHANG))?;
            if syscall::wifexited(status) {
                println!(
                    "Process exited with status {}",
                    syscall::wexitstatus(status)
                );
            }
            if syscall::wifsignaled(status) {
                println!("Process signaled with status {}", syscall::wtermsig(status));
            }
            Ok(())
        },
        other => other,
    }
}
