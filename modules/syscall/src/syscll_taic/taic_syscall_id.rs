//! 记录该模块使用到的系统调用 id
//!
//!

numeric_enum_macro::numeric_enum! {
#[repr(usize)]
#[allow(non_camel_case_types)]
#[allow(missing_docs)]
#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum TaicSyscallId {
    GET_TAIC = 555,
}
}
