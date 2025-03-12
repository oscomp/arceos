use super::*;

#[test]
fn write() {
    assert_eq!(
        syscall_handler(Sysno::write.into(), [
            1,
            "addr to buffer",
            "length of buffer"
        ]),
        0
    );
}
