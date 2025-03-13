//! syscall impl for AstrancE
mod test;

use syscalls::Sysno;
// 声明 axsyscalls 模块
pub mod axsyscalls;
// 声明 sys_fs 模块（对应 sys_fs 目录）
pub mod sys_fs; 
pub use sys_fs::io::*;


pub fn syscall_handler(sys_id: usize, args: [usize; 6]) -> isize {
    let sys_id = Sysno::from(sys_id as u32);//检查id与测例是否适配

    let ret: isize = match sys_id {
        Sysno::write => {
            let fd = args[0];
            let buf_ptr = args[1];
            let size = args[2];
            let buf = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, size) };
            if size == 0 {
                return 0;
            } else {
                sys_write(fd, buf)
            }
        }
        Sysno::read => {
            let fd = args[0];
            let buf_ptr = args[1];
            let size = args[2];
            let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, size) };
            if size == 0 {
                return 0;
            } else {
                sys_read(fd, buf)
            }
        }
        _ => {
            -1 // Return error code for unsupported syscalls
        }
    };

    ret
}
