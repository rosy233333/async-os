#[doc = "Register `add` writer"]
pub type W = crate::W<AddSpec>;
impl core::fmt::Debug for crate::generic::Reg<AddSpec> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "(not readable)")
    }
}
impl W {}
#[doc = "Add task into the priority queue.\n\nYou can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`add::W`](W). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct AddSpec;
impl crate::RegisterSpec for AddSpec {
    type Ux = u64;
}
#[doc = "`write(|w| ..)` method takes [`add::W`](W) writer structure"]
impl crate::Writable for AddSpec {
    type Safety = crate::Unsafe;
    const ZERO_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
    const ONE_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
}
#[doc = "`reset()` method sets add to value 0"]
impl crate::Resettable for AddSpec {
    const RESET_VALUE: u64 = 0;
}
