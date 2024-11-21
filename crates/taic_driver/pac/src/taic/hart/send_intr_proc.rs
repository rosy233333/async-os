#[doc = "Register `send_intr_proc` reader"]
pub type R = crate::R<SendIntrProcSpec>;
#[doc = "Register `send_intr_proc` writer"]
pub type W = crate::W<SendIntrProcSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "send interrupt to the target process.\n\nYou can [`read`](crate::Reg::read) this register and get [`send_intr_proc::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`send_intr_proc::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct SendIntrProcSpec;
impl crate::RegisterSpec for SendIntrProcSpec {
    type Ux = u64;
}
#[doc = "`read()` method returns [`send_intr_proc::R`](R) reader structure"]
impl crate::Readable for SendIntrProcSpec {}
#[doc = "`write(|w| ..)` method takes [`send_intr_proc::W`](W) writer structure"]
impl crate::Writable for SendIntrProcSpec {
    type Safety = crate::Unsafe;
    const ZERO_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
    const ONE_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
}
#[doc = "`reset()` method sets send_intr_proc to value 0"]
impl crate::Resettable for SendIntrProcSpec {
    const RESET_VALUE: u64 = 0;
}
