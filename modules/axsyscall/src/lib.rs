//! syscall impl for AstrancE

#[cfg(test)]
mod test;
use syscalls::Sysno;

/** `add_two` 将指定值加2
*/
pub fn syscall_handler(sys_id: usize, args: [usize; 6]) -> isize {
    // check syscall id 和测力对不对得上
    let sys_id = Sysno::from(sys_id as u32);

    let ret: isize = match sys_id {
        Sysno::write => {
            todo!();
        }
        Sysno::read => {
            todo!();
        }
        _ => {
            todo!();
        }
    };

    ret
}
