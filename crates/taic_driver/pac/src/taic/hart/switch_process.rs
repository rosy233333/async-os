#[doc = "Register `switch_process` reader"]
pub type R = crate::R<SwitchProcessSpec>;
#[doc = "Register `switch_process` writer"]
pub type W = crate::W<SwitchProcessSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Switch process.\n\nYou can [`read`](crate::Reg::read) this register and get [`switch_process::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`switch_process::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct SwitchProcessSpec;
impl crate::RegisterSpec for SwitchProcessSpec {
    type Ux = u64;
}
#[doc = "`read()` method returns [`switch_process::R`](R) reader structure"]
impl crate::Readable for SwitchProcessSpec {}
#[doc = "`write(|w| ..)` method takes [`switch_process::W`](W) writer structure"]
impl crate::Writable for SwitchProcessSpec {
    type Safety = crate::Unsafe;
    const ZERO_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
    const ONE_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
}
#[doc = "`reset()` method sets switch_process to value 0"]
impl crate::Resettable for SwitchProcessSpec {
    const RESET_VALUE: u64 = 0;
}
