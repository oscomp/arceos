use core::arch::asm;

pub const SYS_EXIT: u32 = 93;
pub const SYS_EXIT_GROUP: u32 = 94;
pub const SYS_GETPID: u32 = 172;
pub const SYS_KILL: u32 = 129;
pub const SYS_TGKILL: u32 = 131;
pub const SYS_READ: u32 = 63;
pub const SYS_WRITE: u32 = 64;
pub const SYS_OPENAT: u32 = 56;
pub const SYS_CLOSE: u32 = 57;
pub const SYS_LSEEK: u32 = 62;
pub const SYS_BRK: u32 = 214;
pub const SYS_LINKAT: u32 = 37;
pub const SYS_UNLINKAT: u32 = 35;
pub const SYS_MKDIRAT: u32 = 34;
pub const SYS_RENAMEAT: u32 = 38;
pub const SYS_CHDIR: u32 = 49;
pub const SYS_GETCWD: u32 = 17;
pub const SYS_FSTAT: u32 = 80;
pub const SYS_FSTATAT: u32 = 79;
pub const SYS_FACCESSAT: u32 = 48;
pub const SYS_PREAD: u32 = 67;
pub const SYS_PWRITE: u32 = 68;
pub const SYS_UNAME: u32 = 160;
pub const SYS_GETUID: u32 = 174;
pub const SYS_GETEUID: u32 = 175;
pub const SYS_GETGID: u32 = 176;
pub const SYS_GETEGID: u32 = 177;
pub const SYS_GETTID: u32 = 178;
pub const SYS_SYSINFO: u32 = 179;
pub const SYS_MMAP: u32 = 222;
pub const SYS_MUNMAP: u32 = 215;
pub const SYS_MREMAP: u32 = 216;
pub const SYS_MPROTECT: u32 = 226;
pub const SYS_PRLIMIT64: u32 = 261;
pub const SYS_GETMAINVARS: u32 = 2011;
pub const SYS_RT_SIGACTION: u32 = 134;
pub const SYS_WRITEV: u32 = 66;
pub const SYS_GETTIMEOFDAY: u32 = 169;
pub const SYS_TIMES: u32 = 153;
pub const SYS_FCNTL: u32 = 25;
pub const SYS_FTRUNCATE: u32 = 46;
pub const SYS_GETDENTS: u32 = 61;
pub const SYS_DUP: u32 = 23;
pub const SYS_DUP3: u32 = 24;
pub const SYS_READLINKAT: u32 = 78;
pub const SYS_RT_SIGPROCMASK: u32 = 135;
pub const SYS_IOCTL: u32 = 29;
pub const SYS_GETRLIMIT: u32 = 163;
pub const SYS_SETRLIMIT: u32 = 164;
pub const SYS_GETRUSAGE: u32 = 165;
pub const SYS_CLOCK_GETTIME: u32 = 113;
pub const SYS_SET_TID_ADDRESS: u32 = 96;
pub const SYS_SET_ROBUST_LIST: u32 = 99;
pub const SYS_MADVISE: u32 = 233;
pub const SYS_STATX: u32 = 291;

pub const OLD_SYSCALL_THRESHOLD: u32 = 1024;
pub const SYS_OPEN: u32 = 1024;
pub const SYS_LINK: u32 = 1025;
pub const SYS_UNLINK: u32 = 1026;
pub const SYS_MKDIR: u32 = 1030;
pub const SYS_ACCESS: u32 = 1033;
pub const SYS_STAT: u32 = 1038;
pub const SYS_LSTAT: u32 = 1039;
pub const SYS_TIME: u32 = 1062;



pub fn syscall(id: u32, args: [usize; 6]) -> isize {
    let mut ret: isize;
    unsafe {
        asm!(
            "ecall",
            inlateout("a0") args[0] => ret, // a0 是输入输出寄存器 (x10)
            in("a1") args[1],  // a1 = x11
            in("a2") args[2],  // a2 = x12
            in("a3") args[3],  // a3 = x13
            in("a4") args[4],  // a4 = x14
            in("a5") args[5],  // a5 = x15
            in("a7") id,       // a7 = x17 (系统调用号)
            options(nostack)   // 避免栈操作干扰
        );
    }
    ret
}

