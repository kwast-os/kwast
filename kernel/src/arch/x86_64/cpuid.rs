use bitflags::bitflags;

bitflags! {
    pub struct ExtFeaturesEdx: u32 {
        const GB_PAGES = 1 << 26;
    }
}

extern "C" {
    static CPUID_EXT_EDX: u32;
}

/// Gets the extended features EDX register.
pub fn get_ext_edx() -> ExtFeaturesEdx {
    ExtFeaturesEdx::from_bits_truncate(unsafe { CPUID_EXT_EDX })
}
