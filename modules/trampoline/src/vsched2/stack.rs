use alloc::boxed::Box;

pub fn pick_current_stack() -> TaskStack {
    Box::into_inner(unsafe { Box::from_raw(libvsched2::take_current_stack()) })
}
