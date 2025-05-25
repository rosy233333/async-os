use alloc::{string::String, vec::Vec};
use axerrno::AxResult;
use process::Process;
use taskctx::TaskRef;

pub async fn init_user(args: Vec<String>, envs: &Vec<String>) -> AxResult<TaskRef> {
    Process::init_user(args, envs).await
}
