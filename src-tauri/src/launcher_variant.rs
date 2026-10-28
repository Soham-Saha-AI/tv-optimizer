//! Build-selected SwiftOS APK identity. The candidate hash must be reviewed and
//! entered here before a v3 executable can install or package the candidate.

#[cfg(not(feature = "swiftos-v3-candidate"))]
pub const SWIFTOS_APK_NAME: &str = "liquid-glass.apk";
#[cfg(feature = "swiftos-v3-candidate")]
pub const SWIFTOS_APK_NAME: &str = "swiftos-v3.apk";

#[cfg(not(feature = "swiftos-v3-candidate"))]
pub const SWIFTOS_HASH: &str = "ff6f0c1f93c086e803532f40b7b2165e1c820cd62e588da233fcdf96a4123c06";
#[cfg(feature = "swiftos-v3-candidate")]
pub const SWIFTOS_HASH: &str = "0c86c2542294a5468204acf3bf79c468051d270b6997dde7dabdaf92454a572b";

#[cfg(not(feature = "swiftos-v3-candidate"))]
pub const SWIFTOS_VERSION_CODE: u64 = 22;
#[cfg(feature = "swiftos-v3-candidate")]
pub const SWIFTOS_VERSION_CODE: u64 = 24;

#[cfg(not(feature = "swiftos-v3-candidate"))]
pub const SWIFTOS_VERSION_NAME: &str = "2.7-swiftos";
#[cfg(feature = "swiftos-v3-candidate")]
pub const SWIFTOS_VERSION_NAME: &str = "3.0.1-swiftos";

#[cfg(not(feature = "swiftos-v3-candidate"))]
pub const SWIFTOS_VARIANT: &str = "compatibility-build-22";
#[cfg(feature = "swiftos-v3-candidate")]
pub const SWIFTOS_VARIANT: &str = "v3-local-wallpaper-build-24";

pub fn hash_is_approved() -> bool {
    SWIFTOS_HASH.len() == 64 && !SWIFTOS_HASH.starts_with("PENDING_")
}

