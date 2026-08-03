#[repr(i32)]
#[data]
pub enum LoadError {
    Bad = 1,
}

#[export]
pub trait TypedLoader {
    fn load(&self, key: u32) -> Result<u32, LoadError>;
}
