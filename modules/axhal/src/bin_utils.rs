/// Bit manipulation utilities

/// Set the bit at the specified position to 1.
#[inline]
pub const fn set_bit(bits: &mut usize, position: usize) {
    *bits |= 1 << position;
}

/// Set the bit at the specified position to 0.
#[inline]
pub const fn clear_bit(bits: &mut usize, position: usize) {
    *bits &= !(1 << position);
}

/// Toggle the bit at the specified position.
#[inline]
pub const fn toggle_bit(bits: &mut usize, position: usize) {
    *bits ^= 1 << position;
}

/// Check if the bit at the specified position is set.
#[inline]
pub const fn get_bit(bits: usize, position: usize) -> bool {
    (bits & (1 << position)) != 0
}

/// Get the value of the bits at the specified position and width.
#[inline]
pub const fn get_bits(bits: usize, position: usize, width: usize) -> usize {
    (bits & bit_mask(position, width)) >> position
}

/// Set the value of the bits at the specified position and width.
#[inline]
pub const fn set_bits(bits: &mut usize, position: usize, width: usize, value: usize) {
    let mask = bit_mask(position, width);
    *bits = (*bits & !mask) | ((value << position) & mask);
}

/// generate a bit mask for a given bit and width
pub const fn bit_mask(bit: usize, width: usize) -> usize {
    ((1 << width) - 1) << bit
}