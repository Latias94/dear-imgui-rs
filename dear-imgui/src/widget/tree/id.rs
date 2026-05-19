/// Tree node ID that can be constructed from different types
#[derive(Copy, Clone, Debug)]
pub enum TreeNodeId<T> {
    Str(T),
    Ptr(*const u8),
    Int(i32),
}

impl<T> From<T> for TreeNodeId<T>
where
    T: AsRef<str>,
{
    fn from(s: T) -> Self {
        TreeNodeId::Str(s)
    }
}

impl From<*const u8> for TreeNodeId<&'static str> {
    fn from(ptr: *const u8) -> Self {
        TreeNodeId::Ptr(ptr)
    }
}

impl From<i32> for TreeNodeId<&'static str> {
    fn from(i: i32) -> Self {
        TreeNodeId::Int(i)
    }
}
