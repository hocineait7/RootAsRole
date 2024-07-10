use aya_ebpf::{
    helpers::{bpf_get_current_task, bpf_get_current_uid_gid, bpf_probe_read_kernel},
    macros::map,
    maps::HashMap,
    programs::ProbeContext,
};

use crate::ebpf_util::{get_ns_inode, TaskStructPtr, MAX_PID};

use aya_log_ebpf::{debug, info};

type Key = i32;

#[map]
static mut CAPABILITIES_MAP: HashMap<Key, u64> = HashMap::with_max_entries(MAX_PID, 0);
#[map]
static mut UID_GID_MAP: HashMap<Key, u64> = HashMap::with_max_entries(MAX_PID, 0);
#[map]
static mut PPID_MAP: HashMap<Key, i32> = HashMap::with_max_entries(MAX_PID, 0);
#[map]
static mut PNSID_NSID_MAP: HashMap<Key, u64> = HashMap::with_max_entries(MAX_PID, 0);


pub fn try_capable(ctx: &ProbeContext) -> Result<u32, i64> {
    info!(ctx, "capable");
    unsafe {
        let task: TaskStructPtr = bpf_get_current_task() as TaskStructPtr;
        debug!(ctx, "debug1");
        let task = bpf_probe_read_kernel(&task)?;
        debug!(ctx, "debug2");
        let ppid: i32 = get_ppid(task)?;
        debug!(ctx, "debug3");
        let pid: i32 = bpf_probe_read_kernel(&(*task).pid)? as i32;
        debug!(ctx, "debug4");
        let cap: u64 = (1 << ctx.arg::<u8>(2).unwrap()) as u64;
        debug!(ctx, "debug5");
        let uid: u64 = bpf_get_current_uid_gid();
        debug!(ctx, "debug6");
        let zero = 0;
        let capval: u64 = *CAPABILITIES_MAP.get(&pid).unwrap_or(&zero);
        debug!(ctx, "debug7");
        let pinum_inum: u64 = Into::<u64>::into(get_parent_ns_inode(task)?) << 32
            | Into::<u64>::into(get_ns_inode(task)?);
        debug!(ctx, "debug8");
        UID_GID_MAP
            .insert(&pid, &uid, 0)
            .expect("failed to insert uid");
        debug!(ctx, "debug9");
        PNSID_NSID_MAP
            .insert(&pid, &pinum_inum, 0)
            .expect("failed to insert pnsid");
        debug!(ctx, "debug10");
        PPID_MAP
            .insert(&pid, &ppid, 0)
            .expect("failed to insert ppid");
        debug!(ctx, "debug11");
        CAPABILITIES_MAP
            .insert(&pid, &(capval | cap), 0)
            .expect("failed to insert cap");
    }
    Ok(0)
}

unsafe fn get_ppid(task: TaskStructPtr) -> Result<i32, i64> {
    let parent_task: TaskStructPtr = get_parent_task(task)?;
    bpf_probe_read_kernel(&(*parent_task).pid)
}

unsafe fn get_parent_task(task: TaskStructPtr) -> Result<TaskStructPtr, i64> {
    bpf_probe_read_kernel(&(*task).parent)
}

unsafe fn get_parent_ns_inode(task: TaskStructPtr) -> Result<u32, i64> {
    let parent_task: TaskStructPtr = get_parent_task(task)?;
    get_ns_inode(parent_task)
}


