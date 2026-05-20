use super::*;

impl Io {
    /// Configuration flags
    pub fn config_flags(&self) -> ConfigFlags {
        ConfigFlags::from_bits_retain(self.inner().ConfigFlags)
    }

    /// Set configuration flags
    pub fn set_config_flags(&mut self, flags: ConfigFlags) {
        validate_config_flags("Io::set_config_flags()", flags);
        self.inner_mut().ConfigFlags = flags.bits();
    }
}
