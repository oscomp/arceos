use crate::axsyscalls::*;
pub fn sys_read(fd: usize,buf:&mut[u8]) -> isize {
    syscall(SYS_READ,[fd, buf.as_mut_ptr() as usize, buf.len(),0,0,0])
}
pub fn sys_write(fd: usize,buf:&[u8]) -> isize {
    syscall(SYS_WRITE,[fd, buf.as_ptr() as usize, buf.len(),0,0,0])
}