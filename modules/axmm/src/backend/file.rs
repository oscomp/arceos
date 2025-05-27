use super::Backend;

impl Backend {
    /// Creates a new allocation mapping backend.
    pub const fn new_file(fd: i32, offset: usize, populate: bool, shared: bool) -> Self {
        Self::File { fd, offset, shared, populate}
    }
}
