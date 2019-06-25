# strace for Redox OS

This was made to demonstrate the very basic ptrace-support in
[kernel!103](https://gitlab.redox-os.org/redox-os/kernel/merge_requests/103),
but will hopefully become a useful tool for not only debugging, but
also for creating your very own ptrace-dependent applications.

The core of this program is its own library that will hopefully create
abstractions over all ptrace features some time in the future. The
benefit of letting the `strace` application supply the library is that
you can get debug prints of registers for free. Making this library
cross-platform is not a priority, as everything would have to be
rewritten to use the Linux syscalls one might as well create such a
library for only Linux and then another library which abstracts over
these abstractions...

## Roadmap

- [ ] Memory reading
- [ ] Track subprocesses?
- [ ] (Library) Docs
- [ ] (Library) Support sysemu
