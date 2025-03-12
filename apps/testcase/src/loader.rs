
pub fn load_file(path:&str) -> Result<Vec<u8>, axstd::io::Error> {
    axfs::api::read(path)
}
