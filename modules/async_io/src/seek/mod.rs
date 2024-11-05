mod seek;

use crate::Result;
use alloc::boxed::Box;
use core::{
    ops::DerefMut,
    pin::Pin,
    task::{Context, Poll},
};

/// 异步查找
///
/// 类似于 `std::io::Seek`，但集成了异步任务系统
///
/// `seek` 函数不同于 `std::io::Seek::seek`，当数据还没有准备好时，
/// 当前任务主动让出 CPU
pub trait AsyncSeek {
    /// 尝试从指定位置查找
    ///
    /// 允许超出流的范围，但行为需要自定义
    ///
    /// 如果查找成功，则返回从流的开始处的新位置（后续使用通过 [`SeekFrom::Start`]）
    ///
    /// # 错误
    /// 查找到一个负数位置将视为错误
    ///
    /// # 实现
    ///
    /// 这个函数不会返回 `WouldBlock` 或 `Interrupted` 错误，
    /// 而是将这些错误转化为 `Poll::Pending`，并且在内部进行重试
    /// 或者转化为其他错误
    fn seek(self: Pin<&mut Self>, cx: &mut Context<'_>, pos: SeekFrom) -> Poll<Result<u64>>;
}

macro_rules! deref_async_seek {
    () => {
        fn seek(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            pos: SeekFrom,
        ) -> Poll<Result<u64>> {
            Pin::new(&mut **self).seek(cx, pos)
        }
    };
}

impl<T: ?Sized + AsyncSeek + Unpin> AsyncSeek for Box<T> {
    deref_async_seek!();
}

impl<T: ?Sized + AsyncSeek + Unpin> AsyncSeek for &mut T {
    deref_async_seek!();
}

impl<P> AsyncSeek for Pin<P>
where
    P: DerefMut + Unpin,
    P::Target: AsyncSeek,
{
    fn seek(self: Pin<&mut Self>, cx: &mut Context<'_>, pos: SeekFrom) -> Poll<Result<u64>> {
        self.get_mut().as_mut().seek(cx, pos)
    }
}

/// 枚举在 I/O 对象中寻找的可能方法
///
/// 被 [`Seek`] trait.使用
#[derive(Copy, PartialEq, Eq, Clone, Debug)]
pub enum SeekFrom {
    /// 设置偏移为指定的字节数
    Start(u64),

    /// 将偏移量设置为此对象的大小加上指定的字节数
    ///
    /// 可以搜索到对象末尾以外的位置，但搜索到字节 0 之前的位置则为错误
    End(i64),

    /// 将偏移量设置为当前位置加上指定的字节数
    ///
    /// 可以搜索到对象末尾以外的位置，但搜索到字节 0 之前的位置则为错误
    Current(i64),
}

use seek::SeekFuture;

#[doc = r#"
    Extension methods for [`Seek`].

    [`Seek`]: ../trait.Seek.html
"#]
pub trait Seek: AsyncSeek {
    #[doc = r#"
        Seeks to a new position in a byte stream.

        Returns the new position in the byte stream.

        A seek beyond the end of stream is allowed, but behavior is defined by the
        implementation.

        # Examples

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::fs::File;
        use async_std::io::SeekFrom;
        use async_std::prelude::*;

        let mut file = File::open("a.txt").await?;

        let file_len = file.seek(SeekFrom::End(0)).await?;
        #
        # Ok(()) }) }
        ```
    "#]
    fn seek(&mut self, pos: SeekFrom) -> SeekFuture<'_, Self>
    where
        Self: Unpin,
    {
        SeekFuture { seeker: self, pos }
    }
}

impl<T: AsyncSeek + ?Sized> Seek for T {}
