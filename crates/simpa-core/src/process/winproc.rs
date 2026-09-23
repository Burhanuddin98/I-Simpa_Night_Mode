//! Windows: the child and everything it starts, in one Job Object. The crate's process `unsafe`
//! is confined to this module.
//!
//! The child is created with `CREATE_SUSPENDED | CREATE_NO_WINDOW`, assigned to a fresh job with
//! `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, and only then resumed, so it cannot start a process
//! outside the job. No breakaway flag is set, so its descendants cannot leave the job either.
//! [`Job`] owns the only handle to the job: it is unnamed and not inheritable, so no child holds
//! one. Closing it, by drop or by the kernel when this process dies, kills every process still
//! in the job.
//!
//! One window is left: if this process dies between `CreateProcessW` and
//! `AssignProcessToJobObject` (microseconds; nothing of ours that can fail runs in between), the
//! child stays suspended outside any job. It never runs a user-mode instruction, but it is listed
//! until something kills it. Closing that window needs `PROC_THREAD_ATTRIBUTE_JOB_LIST`, which
//! `std::process::Command` cannot pass on stable Rust. None of the three children starts
//! processes of its own: a grep of upstream `src/{spps,ctr,tetgen,lib_interface}` at 929a5c8 for
//! `CreateProcess`, `ShellExecute`, `WinExec`, `system(`, `popen`, `_spawn`, `fork(`, `exec*`
//! and `boost::process` finds nothing (the same grep finds `wxExecute` in `src/isimpa`).

use std::ffi::c_void;
use std::io;
use std::mem::{offset_of, size_of, size_of_val};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus};
use std::ptr;
use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    ERROR_MORE_DATA, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_BASIC_PROCESS_ID_LIST,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicAccountingInformation,
    JobObjectBasicProcessIdList, JobObjectExtendedLimitInformation, QueryInformationJobObject,
    SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenProcess, OpenThread, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_SYNCHRONIZE, ResumeThread, THREAD_SUSPEND_RESUME, WaitForSingleObject,
};

use super::Tree;

/// Exit code of processes killed by cancel or as stragglers; `Child::kill` uses the same.
const KILLED: u32 = 1;
/// How long killing the tree may take before [`Tree::kill_all`] fails.
const REAP_LIMIT: Duration = Duration::from_secs(10);
/// First guess at the number of processes in a job: the child and room for whatever it starts.
/// (Its console host is not a member: measured, a `ping` child's job lists the child alone.)
const LIST_ROOM: usize = 64;

/// The child, in its job.
pub(super) struct JobTree {
    child: Child,
    job: Job,
}

impl JobTree {
    /// Spawns `command` (stdout and stderr piped) suspended, puts it in a new job, then resumes
    /// it. On any failure after the spawn the child is killed before it has run.
    pub(super) fn spawn(mut command: Command) -> io::Result<(Self, ChildStdout, ChildStderr)> {
        let job = Job::new()?;
        let mut child = command
            .creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW)
            .spawn()?;
        if let Err(e) = job.assign(&child).and_then(|()| resume(child.id())) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = child.stderr.take().expect("stderr is piped");
        Ok((JobTree { child, job }, stdout, stderr))
    }
}

impl Tree for JobTree {
    fn wait_exit(&mut self, timeout: Duration) -> io::Result<Option<ExitStatus>> {
        if !timeout.is_zero() && !wait(self.child.as_raw_handle(), Instant::now() + timeout)? {
            return Ok(None);
        }
        self.child.try_wait()
    }

    /// `TerminateJobObject`, then waits until the job has no active process and every process
    /// seen in it is signalled. Repeats if a process joined the job meanwhile.
    fn kill_all(&mut self) -> io::Result<()> {
        let deadline = Instant::now() + REAP_LIMIT;
        loop {
            // Handles first: an id in the list now cannot pass to a stranger while it is waited on.
            let members = self.job.members()?;
            self.job.terminate()?;
            let mut signalled = true;
            for process in members
                .iter()
                .map(AsRawHandle::as_raw_handle)
                .chain([self.child.as_raw_handle()])
            {
                signalled &= wait(process, deadline)?;
            }
            if signalled && self.job.active_processes()? == 0 {
                break;
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "the child's process tree was still alive {} s after TerminateJobObject",
                        REAP_LIMIT.as_secs()
                    ),
                ));
            }
            thread::sleep(Duration::from_millis(1));
        }
        self.child.try_wait().map(drop)
    }
}

/// The job, owned: dropping it closes the only handle, and `KILL_ON_JOB_CLOSE` then kills every
/// process still in the job. That covers an early return, a panic, and this process dying.
struct Job(OwnedHandle);

impl Job {
    fn new() -> io::Result<Self> {
        // SAFETY: no security attributes (so not inheritable) and no name.
        let raw = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `raw` is a fresh handle that nothing else owns.
        let job = Job(unsafe { OwnedHandle::from_raw_handle(raw) });
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: the pointer and length describe `limits`, which outlives the call.
        check(unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast::<c_void>(),
                size_of_val(&limits) as u32,
            )
        })?;
        Ok(job)
    }

    fn raw(&self) -> HANDLE {
        self.0.as_raw_handle()
    }

    fn assign(&self, child: &Child) -> io::Result<()> {
        // SAFETY: both handles are open; std's process handle has full access.
        check(unsafe { AssignProcessToJobObject(self.raw(), child.as_raw_handle()) })
    }

    fn terminate(&self) -> io::Result<()> {
        // SAFETY: the job handle is open and has full access.
        check(unsafe { TerminateJobObject(self.raw(), KILLED) })
    }

    fn active_processes(&self) -> io::Result<u32> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: the pointer and length describe `info`, which outlives the call.
        check(unsafe {
            QueryInformationJobObject(
                self.raw(),
                JobObjectBasicAccountingInformation,
                (&raw mut info).cast::<c_void>(),
                size_of_val(&info) as u32,
                ptr::null_mut(),
            )
        })?;
        Ok(info.ActiveProcesses)
    }

    /// Ids of the processes in the job, as the kernel lists them now. `room` is the first guess
    /// at their number; the buffer grows when the kernel says it is too small.
    fn process_ids(&self, mut room: usize) -> io::Result<Vec<u32>> {
        // The fixed part of the struct, in `usize` words (the list itself is `usize`-aligned).
        const HEAD: usize =
            offset_of!(JOBOBJECT_BASIC_PROCESS_ID_LIST, ProcessIdList) / size_of::<usize>();
        for _ in 0..8 {
            let mut buf = vec![0usize; HEAD + room];
            let bytes = u32::try_from(size_of_val(buf.as_slice())).map_err(io::Error::other)?;
            // SAFETY: `buf` is `bytes` long, zeroed, and aligned for the struct.
            let ok = unsafe {
                QueryInformationJobObject(
                    self.raw(),
                    JobObjectBasicProcessIdList,
                    buf.as_mut_ptr().cast::<c_void>(),
                    bytes,
                    ptr::null_mut(),
                )
            };
            // SAFETY: `buf` starts with the struct's fixed part, written by the call or still zero.
            let head = unsafe {
                buf.as_ptr()
                    .cast::<JOBOBJECT_BASIC_PROCESS_ID_LIST>()
                    .read()
            };
            if ok != 0 {
                let listed = (head.NumberOfProcessIdsInList as usize).min(room);
                // Process ids fit in 32 bits; the list stores them as `ULONG_PTR`.
                return Ok(buf[HEAD..HEAD + listed].iter().map(|&p| p as u32).collect());
            }
            let e = io::Error::last_os_error();
            if e.raw_os_error() != Some(ERROR_MORE_DATA as i32) {
                return Err(e);
            }
            room = (head.NumberOfAssignedProcesses as usize + 16).max(room * 2);
        }
        Err(io::Error::other(
            "the job's process list kept outgrowing its buffer",
        ))
    }

    /// Handles (`SYNCHRONIZE`) on the processes in the job.
    fn members(&self) -> io::Result<Vec<OwnedHandle>> {
        Ok(self
            .process_ids(LIST_ROOM)?
            .into_iter()
            .filter_map(|pid| self.member(pid))
            .collect())
    }

    /// A handle on process `pid` if it is alive and in the job. An id from the job's list may
    /// already belong to a new process outside it.
    fn member(&self, pid: u32) -> Option<OwnedHandle> {
        let access = PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION;
        // SAFETY: plain call; a null result means the process is gone.
        let raw = unsafe { OpenProcess(access, 0, pid) };
        if raw.is_null() {
            return None;
        }
        // SAFETY: `raw` is a fresh handle that nothing else owns.
        let process = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut inside = 0;
        // SAFETY: both handles are open; `inside` outlives the call.
        let ok = unsafe { IsProcessInJob(process.as_raw_handle(), self.raw(), &mut inside) };
        (ok != 0 && inside != 0).then_some(process)
    }
}

/// Resumes the child's suspended primary thread. `std` closes the thread handle that
/// `CreateProcessW` returned, so the thread is found by a Toolhelp snapshot. Every thread of the
/// process is resumed once; one that was not suspended is left as it was, so a thread injected by
/// another program (a debugger, a security product) does no harm.
fn resume(pid: u32) -> io::Result<()> {
    // SAFETY: plain call; the result is checked.
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `raw` is a fresh handle that nothing else owns.
    let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
    let size = size_of::<THREADENTRY32>() as u32;
    let mut entry = THREADENTRY32 {
        dwSize: size,
        ..Default::default()
    };
    let mut resumed = 0;
    // SAFETY: `entry` is a THREADENTRY32 with `dwSize` set, and outlives the call.
    let mut more = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == pid {
            // SAFETY: plain call; a null result is skipped.
            let raw = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if !raw.is_null() {
                // SAFETY: `raw` is a fresh handle that nothing else owns.
                let thread = unsafe { OwnedHandle::from_raw_handle(raw) };
                // SAFETY: the handle is open with THREAD_SUSPEND_RESUME.
                let before = unsafe { ResumeThread(thread.as_raw_handle()) };
                if before != u32::MAX && before > 0 {
                    resumed += 1;
                }
            }
        }
        entry.dwSize = size;
        // SAFETY: as for `Thread32First`.
        more = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) } != 0;
    }
    if resumed == 0 {
        return Err(io::Error::other(format!(
            "process {pid} was created suspended, but no suspended thread of it was found"
        )));
    }
    Ok(())
}

/// Waits for `process` until `deadline`; true when it is signalled (has exited).
fn wait(process: HANDLE, deadline: Instant) -> io::Result<bool> {
    let left = deadline.saturating_duration_since(Instant::now());
    let ms = u32::try_from(left.as_millis()).unwrap_or(u32::MAX - 1);
    // SAFETY: `process` is an open handle with SYNCHRONIZE access.
    match unsafe { WaitForSingleObject(process, ms) } {
        WAIT_OBJECT_0 => Ok(true),
        WAIT_TIMEOUT => Ok(false),
        _ => Err(io::Error::last_os_error()),
    }
}

fn check(ok: windows_sys::core::BOOL) -> io::Result<()> {
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::process::Stdio;

    use super::*;

    /// `ping -n <n> 127.0.0.1`, piped, running about `n - 1` seconds.
    fn ping(n: u32) -> Command {
        let mut command = Command::new("ping.exe");
        command
            .args(["-n", &n.to_string(), "127.0.0.1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn open(pid: u32) -> OwnedHandle {
        let access = PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION;
        // SAFETY: plain call; the result is checked.
        let raw = unsafe { OpenProcess(access, 0, pid) };
        assert!(!raw.is_null(), "process {pid} is running");
        // SAFETY: `raw` is a fresh handle that nothing else owns.
        unsafe { OwnedHandle::from_raw_handle(raw) }
    }

    #[test]
    fn spawn_runs_the_child_inside_its_job() {
        let (mut tree, _out, _err) = JobTree::spawn(ping(30)).unwrap();
        let pid = tree.child.id();
        assert!(tree.job.member(pid).is_some(), "the child is in the job");
        let ids = tree.job.process_ids(LIST_ROOM).unwrap();
        assert!(ids.contains(&pid), "{ids:?} lists the child {pid}");
        assert!(
            tree.wait_exit(Duration::ZERO).unwrap().is_none(),
            "resumed and running"
        );
        tree.kill_all().unwrap();
        assert_eq!(tree.job.active_processes().unwrap(), 0);
        assert!(tree.wait_exit(Duration::ZERO).unwrap().is_some());
    }

    #[test]
    fn member_refuses_a_process_outside_the_job() {
        let job = Job::new().unwrap();
        let mut outside = ping(30).spawn().unwrap();
        assert!(job.member(outside.id()).is_none());
        job.assign(&outside).unwrap();
        assert!(job.member(outside.id()).is_some());
        job.terminate().unwrap();
        outside.wait().unwrap();
    }

    #[test]
    fn process_ids_grows_past_a_small_buffer() {
        let job = Job::new().unwrap();
        let mut children: Vec<Child> = (0..3).map(|_| ping(30).spawn().unwrap()).collect();
        for child in &children {
            job.assign(child).unwrap();
        }
        let ids = job.process_ids(1).unwrap();
        let want: Vec<u32> = children.iter().map(Child::id).collect();
        assert!(
            want.iter().all(|p| ids.contains(p)),
            "{ids:?} holds {want:?}"
        );
        job.terminate().unwrap();
        for child in &mut children {
            child.wait().unwrap();
        }
    }

    #[test]
    fn resume_refuses_a_process_with_no_suspended_thread() {
        let mut running = ping(30).spawn().unwrap();
        let err = resume(running.id()).unwrap_err();
        assert!(err.to_string().contains("no suspended thread"), "{err}");
        running.kill().unwrap();
        running.wait().unwrap();
    }

    #[test]
    fn dropping_the_tree_kills_the_child() {
        let (tree, _out, _err) = JobTree::spawn(ping(30)).unwrap();
        let child = open(tree.child.id());
        drop(tree);
        let deadline = Instant::now() + Duration::from_secs(2);
        assert!(
            wait(child.as_raw_handle(), deadline).unwrap(),
            "killed on drop"
        );
    }
}
