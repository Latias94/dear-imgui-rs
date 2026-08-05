pub(super) struct StableHash(u64);

impl StableHash {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    pub(super) fn new() -> Self {
        Self(Self::OFFSET)
    }

    pub(super) fn field(&mut self, label: &str, value: &str) {
        self.bytes(b"field");
        self.string(label);
        self.string(value);
    }

    pub(super) fn bool_field(&mut self, label: &str, value: bool) {
        self.field(label, if value { "true" } else { "false" });
    }

    pub(super) fn begin_list(&mut self, label: &str, len: usize) {
        self.bytes(b"list");
        self.string(label);
        self.u64(len as u64);
    }

    pub(super) fn list_item(&mut self, index: usize) {
        self.bytes(b"item");
        self.u64(index as u64);
    }

    pub(super) fn fields<I, S>(&mut self, label: &str, values: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let values = values.into_iter().collect::<Vec<_>>();
        self.begin_list(label, values.len());
        for (index, value) in values.into_iter().enumerate() {
            self.list_item(index);
            self.string(value.as_ref());
        }
    }

    fn string(&mut self, value: &str) {
        self.u64(value.len() as u64);
        self.bytes(value.as_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    pub(super) fn finish(self) -> String {
        format!("fnv1a64:{:016x}", self.0)
    }
}
