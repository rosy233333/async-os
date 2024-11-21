#[doc = "Register `fetch` reader"]
pub type R = crate::R<FetchSpec>;
#[doc = "Field `tcb` reader - The pointer of task control block."]
pub type TcbR = crate::FieldReader<u64>;
impl R {
    #[doc = "Bits 6:64 - The pointer of task control block."]
    #[inline(always)]
    pub fn tcb(&self) -> TcbR {
        TcbR::new((self.bits >> 6) & 0x07ff_ffff_ffff_ffff)
    }
}
#[doc = "Fetch a task from the priority queue.\n\nYou can [`read`](crate::Reg::read) this register and get [`fetch::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct FetchSpec;
impl crate::RegisterSpec for FetchSpec {
    type Ux = u64;
}
#[doc = "`read()` method returns [`fetch::R`](R) reader structure"]
impl crate::Readable for FetchSpec {}
#[doc = "`reset()` method sets fetch to value 0"]
impl crate::Resettable for FetchSpec {
    const RESET_VALUE: u64 = 0;
}
