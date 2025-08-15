use kspin::SpinNoIrq;
use lazyinit::LazyInit;
use ns16550a::Uart;

static UART: LazyInit<SpinNoIrq<Uart>> = LazyInit::new();

/// Writes bytes to the console from input u8 slice.
pub fn write_bytes(bytes: &[u8]) {
    for &c in bytes {
        let uart = UART.lock();
        match c {
            b'\n' => {
                while uart.put(b'\r').is_none() {};
                while uart.put(b'\n').is_none() {};
            }
            c => {
                while uart.put(c).is_none() {};
            }
        }
    }
}

/// Reads bytes from the console into the given mutable slice.
/// Returns the number of bytes read.
pub fn read_bytes(bytes: &mut [u8]) -> usize {
    for (i, byte) in bytes.iter_mut().enumerate() {
        match UART.lock().get() {
            Some(c) => *byte = c,
            None => return i,
        }
    }
    bytes.len()
}

/// Early stage initialization for ns16550a
pub(super) fn init_early() {
    let vaddr = axconfig::devices::UART_PADDR + super::consts::DEVICE_DMW_ADDR;
    let uart = Uart::new(vaddr);
    UART.init_once(SpinNoIrq::new(uart));
}
