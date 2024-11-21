#[doc = "Register `switch_os` reader"]
pub type R = crate::R<SwitchOsSpec>;
#[doc = "Register `switch_os` writer"]
pub type W = crate::W<SwitchOsSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Switch os.\n\nYou can [`read`](crate::Reg::read) this register and get [`switch_os::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`switch_os::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct SwitchOsSpec;
impl crate::RegisterSpec for SwitchOsSpec {
    type Ux = u64;
}
#[doc = "`read()` method returns [`switch_os::R`](R) reader structure"]
impl crate::Readable for SwitchOsSpec {}
#[doc = "`write(|w| ..)` method takes [`switch_os::W`](W) writer structure"]
impl crate::Writable for SwitchOsSpec {
    type Safety = crate::Unsafe;
    const ZERO_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
    const ONE_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
}
#[doc = "`reset()` method sets switch_os to value 0"]
impl crate::Resettable for SwitchOsSpec {
    const RESET_VALUE: u64 = 0;
}
