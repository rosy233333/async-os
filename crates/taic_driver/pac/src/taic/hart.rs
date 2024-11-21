#[repr(C)]
#[doc = "Related registers of one hart"]
#[doc(alias = "hart")]
pub struct Hart {
    add: Add,
    fetch: Fetch,
    switch_process: SwitchProcess,
    switch_os: SwitchOs,
    register_recv_task: RegisterRecvTask,
    register_recv_target_os: RegisterRecvTargetOs,
    register_recv_target_proc: RegisterRecvTargetProc,
    register_recv_target_task: RegisterRecvTargetTask,
    register_send_task: RegisterSendTask,
    register_send_target_os: RegisterSendTargetOs,
    register_send_target_proc: RegisterSendTargetProc,
    register_send_target_task: RegisterSendTargetTask,
    send_intr_os: SendIntrOs,
    send_intr_proc: SendIntrProc,
    send_intr_task: SendIntrTask,
    switch_hypervisor: SwitchHypervisor,
    current: Current,
    remove: Remove,
    status: Status,
    dump: Dump,
}
impl Hart {
    #[doc = "0x00..0x08 - Add task into the priority queue."]
    #[inline(always)]
    pub const fn add(&self) -> &Add {
        &self.add
    }
    #[doc = "0x08..0x10 - Fetch a task from the priority queue."]
    #[inline(always)]
    pub const fn fetch(&self) -> &Fetch {
        &self.fetch
    }
    #[doc = "0x10..0x18 - Switch process."]
    #[inline(always)]
    pub const fn switch_process(&self) -> &SwitchProcess {
        &self.switch_process
    }
    #[doc = "0x18..0x20 - Switch os."]
    #[inline(always)]
    pub const fn switch_os(&self) -> &SwitchOs {
        &self.switch_os
    }
    #[doc = "0x20..0x28 - Register receive task."]
    #[inline(always)]
    pub const fn register_recv_task(&self) -> &RegisterRecvTask {
        &self.register_recv_task
    }
    #[doc = "0x28..0x30 - Register receive target os."]
    #[inline(always)]
    pub const fn register_recv_target_os(&self) -> &RegisterRecvTargetOs {
        &self.register_recv_target_os
    }
    #[doc = "0x30..0x38 - Register receive target process."]
    #[inline(always)]
    pub const fn register_recv_target_proc(&self) -> &RegisterRecvTargetProc {
        &self.register_recv_target_proc
    }
    #[doc = "0x38..0x40 - Register receive target task."]
    #[inline(always)]
    pub const fn register_recv_target_task(&self) -> &RegisterRecvTargetTask {
        &self.register_recv_target_task
    }
    #[doc = "0x40..0x48 - Register send task."]
    #[inline(always)]
    pub const fn register_send_task(&self) -> &RegisterSendTask {
        &self.register_send_task
    }
    #[doc = "0x48..0x50 - Register send target os."]
    #[inline(always)]
    pub const fn register_send_target_os(&self) -> &RegisterSendTargetOs {
        &self.register_send_target_os
    }
    #[doc = "0x50..0x58 - Register send target process."]
    #[inline(always)]
    pub const fn register_send_target_proc(&self) -> &RegisterSendTargetProc {
        &self.register_send_target_proc
    }
    #[doc = "0x58..0x60 - Register send target task."]
    #[inline(always)]
    pub const fn register_send_target_task(&self) -> &RegisterSendTargetTask {
        &self.register_send_target_task
    }
    #[doc = "0x60..0x68 - send interrupt to the target os."]
    #[inline(always)]
    pub const fn send_intr_os(&self) -> &SendIntrOs {
        &self.send_intr_os
    }
    #[doc = "0x68..0x70 - send interrupt to the target process."]
    #[inline(always)]
    pub const fn send_intr_proc(&self) -> &SendIntrProc {
        &self.send_intr_proc
    }
    #[doc = "0x70..0x78 - send interrupt to the target task."]
    #[inline(always)]
    pub const fn send_intr_task(&self) -> &SendIntrTask {
        &self.send_intr_task
    }
    #[doc = "0x78..0x80 - Switch the the hypervisor."]
    #[inline(always)]
    pub const fn switch_hypervisor(&self) -> &SwitchHypervisor {
        &self.switch_hypervisor
    }
    #[doc = "0x80..0x88 - Get the current task."]
    #[inline(always)]
    pub const fn current(&self) -> &Current {
        &self.current
    }
    #[doc = "0x88..0x90 - Remove the specific task."]
    #[inline(always)]
    pub const fn remove(&self) -> &Remove {
        &self.remove
    }
    #[doc = "0x90..0x98 - The status register."]
    #[inline(always)]
    pub const fn status(&self) -> &Status {
        &self.status
    }
    #[doc = "0x98..0xa0 - Dump the information on the specific position."]
    #[inline(always)]
    pub const fn dump(&self) -> &Dump {
        &self.dump
    }
}
#[doc = "add (w) register accessor: Add task into the priority queue.\n\nYou can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`add::W`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@add`]
module"]
#[doc(alias = "add")]
pub type Add = crate::Reg<add::AddSpec>;
#[doc = "Add task into the priority queue."]
pub mod add;
#[doc = "fetch (r) register accessor: Fetch a task from the priority queue.\n\nYou can [`read`](crate::Reg::read) this register and get [`fetch::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@fetch`]
module"]
#[doc(alias = "fetch")]
pub type Fetch = crate::Reg<fetch::FetchSpec>;
#[doc = "Fetch a task from the priority queue."]
pub mod fetch;
#[doc = "switch_process (rw) register accessor: Switch process.\n\nYou can [`read`](crate::Reg::read) this register and get [`switch_process::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`switch_process::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@switch_process`]
module"]
#[doc(alias = "switch_process")]
pub type SwitchProcess = crate::Reg<switch_process::SwitchProcessSpec>;
#[doc = "Switch process."]
pub mod switch_process;
#[doc = "switch_os (rw) register accessor: Switch os.\n\nYou can [`read`](crate::Reg::read) this register and get [`switch_os::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`switch_os::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@switch_os`]
module"]
#[doc(alias = "switch_os")]
pub type SwitchOs = crate::Reg<switch_os::SwitchOsSpec>;
#[doc = "Switch os."]
pub mod switch_os;
#[doc = "register_recv_task (rw) register accessor: Register receive task.\n\nYou can [`read`](crate::Reg::read) this register and get [`register_recv_task::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`register_recv_task::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@register_recv_task`]
module"]
#[doc(alias = "register_recv_task")]
pub type RegisterRecvTask = crate::Reg<register_recv_task::RegisterRecvTaskSpec>;
#[doc = "Register receive task."]
pub mod register_recv_task;
#[doc = "register_recv_target_os (rw) register accessor: Register receive target os.\n\nYou can [`read`](crate::Reg::read) this register and get [`register_recv_target_os::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`register_recv_target_os::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@register_recv_target_os`]
module"]
#[doc(alias = "register_recv_target_os")]
pub type RegisterRecvTargetOs = crate::Reg<register_recv_target_os::RegisterRecvTargetOsSpec>;
#[doc = "Register receive target os."]
pub mod register_recv_target_os;
#[doc = "register_recv_target_proc (rw) register accessor: Register receive target process.\n\nYou can [`read`](crate::Reg::read) this register and get [`register_recv_target_proc::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`register_recv_target_proc::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@register_recv_target_proc`]
module"]
#[doc(alias = "register_recv_target_proc")]
pub type RegisterRecvTargetProc = crate::Reg<register_recv_target_proc::RegisterRecvTargetProcSpec>;
#[doc = "Register receive target process."]
pub mod register_recv_target_proc;
#[doc = "register_recv_target_task (rw) register accessor: Register receive target task.\n\nYou can [`read`](crate::Reg::read) this register and get [`register_recv_target_task::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`register_recv_target_task::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@register_recv_target_task`]
module"]
#[doc(alias = "register_recv_target_task")]
pub type RegisterRecvTargetTask = crate::Reg<register_recv_target_task::RegisterRecvTargetTaskSpec>;
#[doc = "Register receive target task."]
pub mod register_recv_target_task;
#[doc = "register_send_task (rw) register accessor: Register send task.\n\nYou can [`read`](crate::Reg::read) this register and get [`register_send_task::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`register_send_task::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@register_send_task`]
module"]
#[doc(alias = "register_send_task")]
pub type RegisterSendTask = crate::Reg<register_send_task::RegisterSendTaskSpec>;
#[doc = "Register send task."]
pub mod register_send_task;
#[doc = "register_send_target_os (rw) register accessor: Register send target os.\n\nYou can [`read`](crate::Reg::read) this register and get [`register_send_target_os::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`register_send_target_os::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@register_send_target_os`]
module"]
#[doc(alias = "register_send_target_os")]
pub type RegisterSendTargetOs = crate::Reg<register_send_target_os::RegisterSendTargetOsSpec>;
#[doc = "Register send target os."]
pub mod register_send_target_os;
#[doc = "register_send_target_proc (rw) register accessor: Register send target process.\n\nYou can [`read`](crate::Reg::read) this register and get [`register_send_target_proc::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`register_send_target_proc::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@register_send_target_proc`]
module"]
#[doc(alias = "register_send_target_proc")]
pub type RegisterSendTargetProc = crate::Reg<register_send_target_proc::RegisterSendTargetProcSpec>;
#[doc = "Register send target process."]
pub mod register_send_target_proc;
#[doc = "register_send_target_task (rw) register accessor: Register send target task.\n\nYou can [`read`](crate::Reg::read) this register and get [`register_send_target_task::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`register_send_target_task::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@register_send_target_task`]
module"]
#[doc(alias = "register_send_target_task")]
pub type RegisterSendTargetTask = crate::Reg<register_send_target_task::RegisterSendTargetTaskSpec>;
#[doc = "Register send target task."]
pub mod register_send_target_task;
#[doc = "send_intr_os (rw) register accessor: send interrupt to the target os.\n\nYou can [`read`](crate::Reg::read) this register and get [`send_intr_os::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`send_intr_os::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@send_intr_os`]
module"]
#[doc(alias = "send_intr_os")]
pub type SendIntrOs = crate::Reg<send_intr_os::SendIntrOsSpec>;
#[doc = "send interrupt to the target os."]
pub mod send_intr_os;
#[doc = "send_intr_proc (rw) register accessor: send interrupt to the target process.\n\nYou can [`read`](crate::Reg::read) this register and get [`send_intr_proc::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`send_intr_proc::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@send_intr_proc`]
module"]
#[doc(alias = "send_intr_proc")]
pub type SendIntrProc = crate::Reg<send_intr_proc::SendIntrProcSpec>;
#[doc = "send interrupt to the target process."]
pub mod send_intr_proc;
#[doc = "send_intr_task (rw) register accessor: send interrupt to the target task.\n\nYou can [`read`](crate::Reg::read) this register and get [`send_intr_task::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`send_intr_task::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@send_intr_task`]
module"]
#[doc(alias = "send_intr_task")]
pub type SendIntrTask = crate::Reg<send_intr_task::SendIntrTaskSpec>;
#[doc = "send interrupt to the target task."]
pub mod send_intr_task;
#[doc = "switch_hypervisor (rw) register accessor: Switch the the hypervisor.\n\nYou can [`read`](crate::Reg::read) this register and get [`switch_hypervisor::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`switch_hypervisor::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@switch_hypervisor`]
module"]
#[doc(alias = "switch_hypervisor")]
pub type SwitchHypervisor = crate::Reg<switch_hypervisor::SwitchHypervisorSpec>;
#[doc = "Switch the the hypervisor."]
pub mod switch_hypervisor;
#[doc = "current (r) register accessor: Get the current task.\n\nYou can [`read`](crate::Reg::read) this register and get [`current::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@current`]
module"]
#[doc(alias = "current")]
pub type Current = crate::Reg<current::CurrentSpec>;
#[doc = "Get the current task."]
pub mod current;
#[doc = "remove (rw) register accessor: Remove the specific task.\n\nYou can [`read`](crate::Reg::read) this register and get [`remove::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`remove::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@remove`]
module"]
#[doc(alias = "remove")]
pub type Remove = crate::Reg<remove::RemoveSpec>;
#[doc = "Remove the specific task."]
pub mod remove;
#[doc = "status (r) register accessor: The status register.\n\nYou can [`read`](crate::Reg::read) this register and get [`status::R`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@status`]
module"]
#[doc(alias = "status")]
pub type Status = crate::Reg<status::StatusSpec>;
#[doc = "The status register."]
pub mod status;
#[doc = "dump (rw) register accessor: Dump the information on the specific position.\n\nYou can [`read`](crate::Reg::read) this register and get [`dump::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`dump::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@dump`]
module"]
#[doc(alias = "dump")]
pub type Dump = crate::Reg<dump::DumpSpec>;
#[doc = "Dump the information on the specific position."]
pub mod dump;
