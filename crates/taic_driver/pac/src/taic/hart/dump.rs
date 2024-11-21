#[doc = "Register `dump` reader"]
pub type R = crate::R<DumpSpec>;
#[doc = "Register `dump` writer"]
pub type W = crate::W<DumpSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Dump the information on the specific position.\n\nYou can [`read`](crate::Reg::read) this register and get [`dump::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dump::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct DumpSpec;
impl crate::RegisterSpec for DumpSpec {
    type Ux = u64;
}
#[doc = "`read()` method returns [`dump::R`](R) reader structure"]
impl crate::Readable for DumpSpec {}
#[doc = "`write(|w| ..)` method takes [`dump::W`](W) writer structure"]
impl crate::Writable for DumpSpec {
    type Safety = crate::Unsafe;
    const ZERO_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
    const ONE_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
}
#[doc = "`reset()` method sets dump to value 0"]
impl crate::Resettable for DumpSpec {
    const RESET_VALUE: u64 = 0;
}
