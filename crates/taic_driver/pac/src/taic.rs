#[repr(C)]
#[doc = "Register block"]
pub struct RegisterBlock {
    harts: (),
}
impl RegisterBlock {
    #[doc = "0x00..0xa000 - Related registers of one hart"]
    #[inline(always)]
    pub const fn harts(&self, n: usize) -> &Hart {
        #[allow(clippy::no_effect)]
        [(); 256][n];
        unsafe { &*core::ptr::from_ref(self).cast::<u8>().add(4096 * n).cast() }
    }
    #[doc = "Iterator for array of:"]
    #[doc = "0x00..0xa000 - Related registers of one hart"]
    #[inline(always)]
    pub fn harts_iter(&self) -> impl Iterator<Item = &Hart> {
        (0..256)
            .map(move |n| unsafe { &*core::ptr::from_ref(self).cast::<u8>().add(4096 * n).cast() })
    }
}
#[doc = "Related registers of one hart"]
pub use self::hart::Hart;
#[doc = r"Cluster"]
#[doc = "Related registers of one hart"]
pub mod hart;
