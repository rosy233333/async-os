#[doc = "Register `current` reader"]
pub type R = crate::R<CurrentSpec>;
#[doc = "Field `tcb` reader - The pointer of task control block."]
pub type TcbR = crate::FieldReader<u64>;
impl R {
    #[doc = "Bits 6:64 - The pointer of task control block."]
    #[inline(always)]
    pub fn tcb(&self) -> TcbR {
        TcbR::new((self.bits >> 6) & 0x07ff_ffff_ffff_ffff)
    }
}
#[doc = "Get the current task.\n\nYou can [`read`](crate::Reg::read) this register and get [`current::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct CurrentSpec;
impl crate::RegisterSpec for CurrentSpec {
    type Ux = u64;
}
#[doc = "`read()` method returns [`current::R`](R) reader structure"]
impl crate::Readable for CurrentSpec {}
#[doc = "`reset()` method sets current to value 0"]
impl crate::Resettable for CurrentSpec {
    const RESET_VALUE: u64 = 0;
}
