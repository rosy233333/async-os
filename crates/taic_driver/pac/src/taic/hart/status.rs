#[doc = "Register `status` reader"]
pub type R = crate::R<StatusSpec>;
#[doc = "Field `cause` reader - The cause of interrupt."]
pub type CauseR = crate::FieldReader;
#[doc = "Field `ocnt` reader - The online hart of the current os/process."]
pub type OcntR = crate::FieldReader<u64>;
impl R {
    #[doc = "Bits 0:3 - The cause of interrupt."]
    #[inline(always)]
    pub fn cause(&self) -> CauseR {
        CauseR::new((self.bits & 0x0f) as u8)
    }
    #[doc = "Bits 4:64 - The online hart of the current os/process."]
    #[inline(always)]
    pub fn ocnt(&self) -> OcntR {
        OcntR::new((self.bits >> 4) & 0x1fff_ffff_ffff_ffff)
    }
}
#[doc = "The status register.\n\nYou can [`read`](crate::Reg::read) this register and get [`status::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct StatusSpec;
impl crate::RegisterSpec for StatusSpec {
    type Ux = u64;
}
#[doc = "`read()` method returns [`status::R`](R) reader structure"]
impl crate::Readable for StatusSpec {}
#[doc = "`reset()` method sets status to value 0"]
impl crate::Resettable for StatusSpec {
    const RESET_VALUE: u64 = 0;
}
