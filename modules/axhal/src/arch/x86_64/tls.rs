#[cfg(feature = "uspace")]
mod uspace {
    use crate::arch::{TrapFrame, read_thread_pointer, write_thread_pointer};

    #[cfg(feature = "tls")]
    #[unsafe(no_mangle)]
    #[percpu::def_percpu]
    static KERNEL_FS_BASE: usize = 0;

    #[unsafe(no_mangle)]
    fn switch_to_kernel_fs_base(tf: &mut TrapFrame) {
        tf.fs_base = read_thread_pointer() as _;
        #[cfg(feature = "tls")]
        unsafe {
            write_thread_pointer(KERNEL_FS_BASE.read_current())
        };
    }

    #[unsafe(no_mangle)]
    pub fn switch_to_user_fs_base(tf: &TrapFrame) {
        #[cfg(feature = "tls")]
        KERNEL_FS_BASE.write_current(read_thread_pointer());
        unsafe { write_thread_pointer(tf.fs_base as _) };
    }
}

#[cfg(feature = "uspace")]
pub(super) use uspace::switch_to_user_fs_base;

#[cfg(not(feature = "uspace"))]
#[unsafe(no_mangle)]
fn switch_to_kernel_fs_base(_tf: &mut super::TrapFrame) {}

#[cfg(not(feature = "uspace"))]
#[unsafe(no_mangle)]
pub(super) fn switch_to_user_fs_base(_tf: &super::TrapFrame) {}
