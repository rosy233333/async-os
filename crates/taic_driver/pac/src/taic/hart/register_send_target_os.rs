#[doc = "Register `register_send_target_os` reader"]
pub type R = crate::R<RegisterSendTargetOsSpec>;
#[doc = "Register `register_send_target_os` writer"]
pub type W = crate::W<RegisterSendTargetOsSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Register send target os.\n\nYou can [`read`](crate::Reg::read) this register and get [`register_send_target_os::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`register_send_target_os::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct RegisterSendTargetOsSpec;
impl crate::RegisterSpec for RegisterSendTargetOsSpec {
    type Ux = u64;
}
#[doc = "`read()` method returns [`register_send_target_os::R`](R) reader structure"]
impl crate::Readable for RegisterSendTargetOsSpec {}
#[doc = "`write(|w| ..)` method takes [`register_send_target_os::W`](W) writer structure"]
impl crate::Writable for RegisterSendTargetOsSpec {
    type Safety = crate::Unsafe;
    const ZERO_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
    const ONE_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
}
#[doc = "`reset()` method sets register_send_target_os to value 0"]
impl crate::Resettable for RegisterSendTargetOsSpec {
    const RESET_VALUE: u64 = 0;
}
