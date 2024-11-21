#[doc = "Register `register_recv_target_proc` reader"]
pub type R = crate::R<RegisterRecvTargetProcSpec>;
#[doc = "Register `register_recv_target_proc` writer"]
pub type W = crate::W<RegisterRecvTargetProcSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
impl W {}
#[doc = "Register receive target process.\n\nYou can [`read`](crate::Reg::read) this register and get [`register_recv_target_proc::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`register_recv_target_proc::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct RegisterRecvTargetProcSpec;
impl crate::RegisterSpec for RegisterRecvTargetProcSpec {
    type Ux = u64;
}
#[doc = "`read()` method returns [`register_recv_target_proc::R`](R) reader structure"]
impl crate::Readable for RegisterRecvTargetProcSpec {}
#[doc = "`write(|w| ..)` method takes [`register_recv_target_proc::W`](W) writer structure"]
impl crate::Writable for RegisterRecvTargetProcSpec {
    type Safety = crate::Unsafe;
    const ZERO_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
    const ONE_TO_MODIFY_FIELDS_BITMAP: u64 = 0;
}
#[doc = "`reset()` method sets register_recv_target_proc to value 0"]
impl crate::Resettable for RegisterRecvTargetProcSpec {
    const RESET_VALUE: u64 = 0;
}
