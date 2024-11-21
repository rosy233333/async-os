#[doc = "Register `send_intr_os` reader"]
pub type R = crate::R<SendIntrOsSpec>;
#[doc = "Register `send_intr_os` writer"]
pub type W = crate::W<SendIntrOsSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "send interrupt to the target os.\n\nYou can [`read`](crate::Reg::read) this register and get [`send_intr_os::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`send_intr_os::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct SendIntrOsSpec;
impl crate::RegisterSpec for SendIntrOsSpec {
    type Ux = u64;
}
#[doc = "`read()` method returns [`send_intr_os::R`](R) reader structure"]
impl crate::Readable for SendIntrOsSpec {}
#[doc = "`write(|w| ..)` method takes [`send_intr_os::W`](W) writer structure"]
impl crate::Writable for SendIntrOsSpec {
    type Safety = crate::Unsafe;
    const ZERO_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
    const ONE_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
}
#[doc = "`reset()` method sets send_intr_os to value 0"]
impl crate::Resettable for SendIntrOsSpec {
    const RESET_VALUE: u64 = 0;
}
