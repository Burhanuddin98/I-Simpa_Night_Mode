//! R3's delete: a folder moved to the Recycle Bin, or nothing done at all.
//!
//! The shell's own file operation (`IFileOperation`) does the move, asked to recycle
//! (`FOF_ALLOWUNDO`, `FOFX_RECYCLEONDELETE`). Asked to recycle, the shell still deletes outright
//! whatever it cannot recycle: a folder larger than the drive's Recycle Bin may hold, a drive
//! whose Recycle Bin is turned off, a network or removable drive. Three guards stand between a
//! run and that:
//!
//! 1. Before the shell is asked, the drive's own Recycle Bin settings are read ([`BinPolicy`]:
//!    the per-volume `NukeOnDelete` and `MaxCapacity` under `HKCU\...\Explorer\BitBucket\Volume\
//!    {GUID}`, the `BitBucket` key's values where the volume has none, Windows' default size where
//!    neither has one) and the run's size measured; a drive with no bin, a bin turned off, or a run
//!    larger than the bin holds is refused ([`bin_holds`]) and nothing is asked of the shell.
//! 2. The shell is allowed its one question (no `FOF_NOCONFIRMATION`, and `FOF_WANTNUKEWARNING`):
//!    if guard 1 ever misses a case, Windows' own "permanently delete?" dialog is put, never a
//!    silent permanent delete, and a No there is a refusal ([`Refusal::Declined`]).
//! 3. A progress sink watches every item before the shell touches it: an item that the shell
//!    says it is not going to recycle (`PreDeleteItem` without `TSF_DELETE_RECYCLE_IF_POSSIBLE`)
//!    aborts the operation. That flag echoes the policy asked for and is set before the shell
//!    checks the bin, so it is the last guard, not the first.
//!
//! Afterwards the move counts only when the shell reported no error and nothing aborted, the
//! folder is gone, and the shell named the item it made in the Recycle Bin (`PostDeleteItem`'s
//! new item); anything else is said, never taken as done.

use std::path::Path;

/// Why a folder was not moved to the Recycle Bin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Windows would have deleted it outright (no Recycle Bin for it, the bin off, the run larger
    /// than the bin); nothing was deleted. The sentence says which.
    NotRecyclable(String),
    /// Windows asked whether to go on and the answer was No; nothing was deleted.
    Declined,
    /// The move failed or was not confirmed; the sentence says what was seen.
    Failed(String),
}

/// One key's Recycle Bin values, each absent when the key does not hold it.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinValues {
    /// `NukeOnDelete`: 1 when files on the drive are deleted at once, never recycled.
    #[serde(rename = "NukeOnDelete", default)]
    pub nuke_on_delete: Option<u32>,
    /// `MaxCapacity`: the most the bin holds, in MB (MiB).
    #[serde(rename = "MaxCapacity", default)]
    pub max_capacity_mb: Option<u32>,
}

/// The Recycle Bin settings for the drive a folder is on, as Windows keeps them.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinPolicy {
    /// The drive's root as named in the sentences (`B:\`, `\\server\share\`).
    #[serde(default)]
    pub root: String,
    /// The drive keeps a Recycle Bin at all: a local fixed drive with a volume name. A network,
    /// removable, optical or substituted drive has none.
    pub has_bin: bool,
    /// The volume's size in MB, for Windows' default bin size.
    #[serde(default)]
    pub volume_mb: u64,
    /// The volume's own key, `BitBucket\Volume\{GUID}`.
    #[serde(default)]
    pub volume: BinValues,
    /// The `BitBucket` key's values, used where the volume's key has none.
    #[serde(default)]
    pub global: BinValues,
    /// The group policy "do not move deleted files to the Recycle Bin" (`NoRecycleFiles`).
    #[serde(default)]
    pub no_recycle_files: bool,
}

/// Windows' own bin size where no key gives one: a tenth of the first 40 GB and a twentieth of the
/// rest, in MB. Read off this machine's keys, which Windows wrote: C: 306412 MB -> 17368,
/// F: 3815413 MB -> 192818, B: 488353 MB -> 26465.
pub fn default_capacity_mb(volume_mb: u64) -> u64 {
    const FIRST: u64 = 40 * 1024;
    if volume_mb <= FIRST {
        volume_mb / 10
    } else {
        FIRST / 10 + (volume_mb - FIRST) / 20
    }
}

/// Guard 1's decision: whether the drive's Recycle Bin would take a run of `run_bytes`. `Err`
/// is the plain sentence of why not.
pub fn bin_holds(policy: &BinPolicy, run_bytes: u64) -> Result<(), String> {
    let drive = if policy.root.is_empty() {
        "its drive".to_string()
    } else {
        format!("drive {}", policy.root)
    };
    if !policy.has_bin {
        return Err(format!(
            "{drive} has no Recycle Bin (a network, removable or substituted drive)"
        ));
    }
    let nuke = policy
        .volume
        .nuke_on_delete
        .or(policy.global.nuke_on_delete)
        .unwrap_or(0);
    if nuke != 0 || policy.no_recycle_files {
        return Err(format!(
            "the Recycle Bin of {drive} is turned off, so Windows would delete the run at once"
        ));
    }
    let cap_mb = policy
        .volume
        .max_capacity_mb
        .or(policy.global.max_capacity_mb)
        .map(u64::from)
        .unwrap_or_else(|| default_capacity_mb(policy.volume_mb));
    if run_bytes > cap_mb.saturating_mul(1 << 20) {
        return Err(format!(
            "the run is {} MB, more than the {cap_mb} MB the Recycle Bin of {drive} holds",
            run_bytes.div_ceil(1 << 20)
        ));
    }
    Ok(())
}

/// A fake policy for a test of the refusal in the app, from the JSON file this variable names
/// ([`BinPolicy`]'s fields). It is checked after the drive's real one and can only add a refusal,
/// never lift one, so it cannot open a way to a permanent delete.
pub const FAKE_POLICY_ENV: &str = "SIMPA_BIN_POLICY";

fn fake_policy() -> Option<Result<BinPolicy, String>> {
    let path = std::env::var_os(FAKE_POLICY_ENV).filter(|p| !p.is_empty())?;
    let path = Path::new(&path);
    Some(
        std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
            .map_err(|e| {
                format!(
                    "the Recycle Bin policy in {} does not read: {e}",
                    path.display()
                )
            }),
    )
}

/// The bytes of every file under `dir`, links not followed (the shell moves a link, not what it
/// points at).
fn tree_bytes(dir: &Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d)? {
            let entry = entry?;
            let meta = std::fs::symlink_metadata(entry.path())?;
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}

/// Guard 1 with the policy readers given: the drive's real one, then any fake one.
fn guard(
    dir: &Path,
    real: impl Fn(&Path) -> Result<BinPolicy, String>,
    fake: Option<Result<BinPolicy, String>>,
) -> Result<(), Refusal> {
    let bytes = tree_bytes(dir).map_err(|e| {
        Refusal::Failed(format!("the size of {} does not read: {e}", dir.display()))
    })?;
    let policies = std::iter::once(real(dir)).chain(fake);
    for policy in policies {
        let policy = policy.map_err(Refusal::Failed)?;
        bin_holds(&policy, bytes).map_err(Refusal::NotRecyclable)?;
    }
    Ok(())
}

/// Moves the folder `dir` (absolute) to the Recycle Bin, or refuses and leaves it in place.
pub fn to_recycle_bin(dir: &Path) -> Result<(), Refusal> {
    if !dir.is_dir() {
        return Err(Refusal::Failed(format!(
            "{} is not a folder",
            dir.display()
        )));
    }
    guard(dir, imp::policy_of, fake_policy())?;
    let dir = dir.to_path_buf();
    // The shell's file operation wants a single-threaded apartment: a thread of its own, so no
    // pool thread's COM state is assumed or changed.
    std::thread::spawn(move || imp::recycle(&dir))
        .join()
        .unwrap_or_else(|_| {
            Err(Refusal::Failed(
                "the Recycle Bin move stopped unexpectedly".to_string(),
            ))
        })
}

#[cfg(not(windows))]
mod imp {
    use super::{BinPolicy, Refusal};

    pub fn policy_of(_: &std::path::Path) -> Result<BinPolicy, String> {
        Ok(BinPolicy::default())
    }

    pub fn recycle(_: &std::path::Path) -> Result<(), Refusal> {
        Err(Refusal::Failed(
            "a run is moved to the Recycle Bin on Windows only".to_string(),
        ))
    }
}

#[cfg(windows)]
mod imp {
    use std::cell::RefCell;
    use std::path::Path;
    use std::rc::Rc;

    use windows::Win32::Foundation::{
        E_ABORT, ERROR_CANCELLED, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS,
    };
    use windows::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetVolumeNameForVolumeMountPointW, GetVolumePathNameW,
    };
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
        CoInitializeEx, CoUninitialize,
    };
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RegGetValueW,
    };
    use windows::Win32::UI::Shell::{
        COPYENGINE_E_USER_CANCELLED, FOF_ALLOWUNDO, FOF_NOERRORUI, FOF_SILENT, FOF_WANTNUKEWARNING,
        FOFX_EARLYFAILURE, FOFX_RECYCLEONDELETE, FileOperation, IFileOperation,
        IFileOperationProgressSink, IFileOperationProgressSink_Impl, IShellItem,
        SHCreateItemFromParsingName, TSF_DELETE_RECYCLE_IF_POSSIBLE,
    };
    use windows::core::{HRESULT, HSTRING, PCWSTR, Ref, implement};

    use super::{BinPolicy, BinValues, Refusal};

    /// `GetDriveTypeW`'s answer for a local fixed drive, the only kind that keeps a Recycle Bin.
    const DRIVE_FIXED: u32 = 3;
    const BITBUCKET: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket";
    const POLICIES: &str = r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer";

    fn until_nul(buf: &[u16]) -> String {
        let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..n])
    }

    /// One DWORD under `key\sub`, `None` when the key or the value is not there; any other answer
    /// (a value of another type, a key that does not open) is an error, never taken as absent.
    fn dword(key: HKEY, sub: &str, value: &str) -> Result<Option<u32>, String> {
        let mut data = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        // SAFETY: `data` and `size` are live locals of the sizes passed; the strings outlive the call.
        let r = unsafe {
            RegGetValueW(
                key,
                &HSTRING::from(sub),
                &HSTRING::from(value),
                RRF_RT_REG_DWORD,
                None,
                Some((&mut data as *mut u32).cast()),
                Some(&mut size),
            )
        };
        if r == ERROR_SUCCESS {
            Ok(Some(data))
        } else if r == ERROR_FILE_NOT_FOUND {
            Ok(None)
        } else {
            Err(format!(
                "the Recycle Bin setting {value} under {sub} does not read (error {})",
                r.0
            ))
        }
    }

    fn values(sub: &str) -> Result<BinValues, String> {
        Ok(BinValues {
            nuke_on_delete: dword(HKEY_CURRENT_USER, sub, "NukeOnDelete")?,
            max_capacity_mb: dword(HKEY_CURRENT_USER, sub, "MaxCapacity")?,
        })
    }

    /// The Recycle Bin settings of the drive `dir` is on, read from Windows: the drive's kind,
    /// its volume's name and size, its `BitBucket` keys and the group policy.
    pub fn policy_of(dir: &Path) -> Result<BinPolicy, String> {
        let path = HSTRING::from(dir.as_os_str());
        let mut buf = [0u16; 1024];
        // SAFETY: plain calls with live buffers of the lengths passed, strings that outlive them.
        unsafe { GetVolumePathNameW(&path, &mut buf) }.map_err(|e| {
            format!(
                "the drive of {} is not found ({})",
                dir.display(),
                e.message()
            )
        })?;
        let root = until_nul(&buf);
        let root_h = HSTRING::from(root.as_str());
        let mut policy = BinPolicy {
            root: root.clone(),
            ..BinPolicy::default()
        };
        // SAFETY: as above.
        if unsafe { GetDriveTypeW(&root_h) } != DRIVE_FIXED {
            return Ok(policy);
        }
        let mut name = [0u16; 64];
        // SAFETY: as above. A drive with no volume name (a substituted one) keeps no bin.
        if unsafe { GetVolumeNameForVolumeMountPointW(&root_h, &mut name) }.is_err() {
            return Ok(policy);
        }
        let name = until_nul(&name);
        let Some(guid) = name
            .find('{')
            .and_then(|a| name[a..].find('}').map(|b| &name[a..=a + b]))
        else {
            return Ok(policy);
        };
        let mut total = 0u64;
        // SAFETY: as above.
        unsafe { GetDiskFreeSpaceExW(&root_h, None, Some(&mut total), None) }
            .map_err(|e| format!("the size of drive {root} does not read ({})", e.message()))?;
        policy.has_bin = true;
        policy.volume_mb = total >> 20;
        policy.volume = values(&format!(r"{BITBUCKET}\Volume\{guid}"))?;
        policy.global = values(BITBUCKET)?;
        policy.no_recycle_files = [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
            .into_iter()
            .map(|k| dword(k, POLICIES, "NoRecycleFiles"))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .any(|v| v.is_some_and(|v| v != 0));
        Ok(policy)
    }

    /// What the shell said about each item, as it went.
    #[derive(Default)]
    struct Seen {
        /// Items the shell was about to delete without recycling (each one refused).
        outright: u32,
        /// Items it was about to recycle.
        recycling: u32,
        /// `PostDeleteItem`: the result, and whether it named the item now in the Recycle Bin.
        done: Vec<(HRESULT, bool)>,
    }

    #[implement(IFileOperationProgressSink)]
    struct Sink {
        seen: Rc<RefCell<Seen>>,
    }

    #[allow(non_snake_case)]
    impl IFileOperationProgressSink_Impl for Sink_Impl {
        fn StartOperations(&self) -> windows::core::Result<()> {
            Ok(())
        }
        fn FinishOperations(&self, _: HRESULT) -> windows::core::Result<()> {
            Ok(())
        }
        fn PreRenameItem(
            &self,
            _: u32,
            _: Ref<'_, IShellItem>,
            _: &PCWSTR,
        ) -> windows::core::Result<()> {
            Ok(())
        }
        fn PostRenameItem(
            &self,
            _: u32,
            _: Ref<'_, IShellItem>,
            _: &PCWSTR,
            _: HRESULT,
            _: Ref<'_, IShellItem>,
        ) -> windows::core::Result<()> {
            Ok(())
        }
        fn PreMoveItem(
            &self,
            _: u32,
            _: Ref<'_, IShellItem>,
            _: Ref<'_, IShellItem>,
            _: &PCWSTR,
        ) -> windows::core::Result<()> {
            Ok(())
        }
        fn PostMoveItem(
            &self,
            _: u32,
            _: Ref<'_, IShellItem>,
            _: Ref<'_, IShellItem>,
            _: &PCWSTR,
            _: HRESULT,
            _: Ref<'_, IShellItem>,
        ) -> windows::core::Result<()> {
            Ok(())
        }
        fn PreCopyItem(
            &self,
            _: u32,
            _: Ref<'_, IShellItem>,
            _: Ref<'_, IShellItem>,
            _: &PCWSTR,
        ) -> windows::core::Result<()> {
            Ok(())
        }
        fn PostCopyItem(
            &self,
            _: u32,
            _: Ref<'_, IShellItem>,
            _: Ref<'_, IShellItem>,
            _: &PCWSTR,
            _: HRESULT,
            _: Ref<'_, IShellItem>,
        ) -> windows::core::Result<()> {
            Ok(())
        }
        /// The last veto: an item the shell says it is not going to recycle stops the operation
        /// before it is touched. The flag echoes the policy asked for, so guard 1 comes first.
        fn PreDeleteItem(&self, dwflags: u32, _: Ref<'_, IShellItem>) -> windows::core::Result<()> {
            let mut seen = self.seen.borrow_mut();
            if dwflags & TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32 == 0 {
                seen.outright += 1;
                return Err(E_ABORT.into());
            }
            seen.recycling += 1;
            Ok(())
        }
        fn PostDeleteItem(
            &self,
            _: u32,
            _: Ref<'_, IShellItem>,
            hrdelete: HRESULT,
            psinewlycreated: Ref<'_, IShellItem>,
        ) -> windows::core::Result<()> {
            self.seen
                .borrow_mut()
                .done
                .push((hrdelete, psinewlycreated.is_some()));
            Ok(())
        }
        fn PreNewItem(
            &self,
            _: u32,
            _: Ref<'_, IShellItem>,
            _: &PCWSTR,
        ) -> windows::core::Result<()> {
            Ok(())
        }
        fn PostNewItem(
            &self,
            _: u32,
            _: Ref<'_, IShellItem>,
            _: &PCWSTR,
            _: &PCWSTR,
            _: u32,
            _: HRESULT,
            _: Ref<'_, IShellItem>,
        ) -> windows::core::Result<()> {
            Ok(())
        }
        fn UpdateProgress(&self, _: u32, _: u32) -> windows::core::Result<()> {
            Ok(())
        }
        fn ResetTimer(&self) -> windows::core::Result<()> {
            Ok(())
        }
        fn PauseTimer(&self) -> windows::core::Result<()> {
            Ok(())
        }
        fn ResumeTimer(&self) -> windows::core::Result<()> {
            Ok(())
        }
    }

    /// The shell's result: whether it returned an error, whether anything aborted.
    struct Ran {
        result: windows::core::Result<()>,
        aborted: bool,
    }

    fn failed(what: &str, e: &windows::core::Error) -> Refusal {
        Refusal::Failed(format!("{what} ({:#010x}: {})", e.code().0, e.message()))
    }

    /// Runs on a thread of its own (`to_recycle_bin`).
    pub fn recycle(dir: &Path) -> Result<(), Refusal> {
        // SAFETY: COM is initialised on this thread, which is ours alone, and uninitialised below
        // once every COM object made here has been dropped (`shell` returns none).
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }
            .ok()
            .map_err(|e| failed("the shell's file operation did not start", &e))?;
        let seen = Rc::new(RefCell::new(Seen::default()));
        let ran = shell(dir, &seen);
        // SAFETY: paired with the successful CoInitializeEx above, on the same thread.
        unsafe { CoUninitialize() };
        let ran = ran?;
        verdict(dir, &seen.borrow(), &ran)
    }

    /// Asks the shell to move `dir` to the Recycle Bin, the sink watching.
    fn shell(dir: &Path, seen: &Rc<RefCell<Seen>>) -> Result<Ran, Refusal> {
        let path = HSTRING::from(dir.as_os_str());
        // SAFETY: plain COM calls on interfaces made here, on an initialised STA thread; every
        // pointer passed is a live interface or a string that outlives the call.
        unsafe {
            let op: IFileOperation =
                CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)
                    .map_err(|e| failed("the shell's file operation did not start", &e))?;
            // No progress window and no error boxes, but never FOF_NOCONFIRMATION: a permanent
            // delete the guards missed is put to the user in Windows' own dialog, never done
            // silently.
            op.SetOperationFlags(
                FOF_SILENT
                    | FOF_NOERRORUI
                    | FOF_ALLOWUNDO
                    | FOF_WANTNUKEWARNING
                    | FOFX_RECYCLEONDELETE
                    | FOFX_EARLYFAILURE,
            )
            .map_err(|e| failed("the shell refused the Recycle Bin flags", &e))?;
            let item: IShellItem = SHCreateItemFromParsingName(&path, None)
                .map_err(|e| failed(&format!("the shell does not find {}", dir.display()), &e))?;
            let sink: IFileOperationProgressSink = Sink { seen: seen.clone() }.into();
            let cookie = op
                .Advise(&sink)
                .map_err(|e| failed("the shell did not take the progress watch", &e))?;
            let result = op
                .DeleteItem(&item, None)
                .and_then(|()| op.PerformOperations());
            let aborted = op
                .GetAnyOperationsAborted()
                .map(|b| b.as_bool())
                .unwrap_or(true);
            let _ = op.Unadvise(cookie);
            Ok(Ran { result, aborted })
        }
    }

    /// Done only when the shell reported no error, nothing aborted, every item it touched went to
    /// the Recycle Bin and named its new place there, and the folder is gone.
    fn verdict(dir: &Path, seen: &Seen, ran: &Ran) -> Result<(), Refusal> {
        let still_there = dir.exists();
        if seen.outright > 0 {
            return if still_there {
                Err(Refusal::NotRecyclable(
                    "Windows said it would delete the run outright rather than put it in the \
                     Recycle Bin"
                        .to_string(),
                ))
            } else {
                // Not expected: the veto comes before the shell acts. Said, never hidden.
                Err(Refusal::Failed(format!(
                    "the shell would not recycle {}, and it is no longer there",
                    dir.display()
                )))
            };
        }
        // A No in Windows' own dialog: the shell answers "cancelled", or stops with no error.
        let cancelled = match &ran.result {
            Err(e) => {
                e.code() == COPYENGINE_E_USER_CANCELLED || e.code() == ERROR_CANCELLED.to_hresult()
            }
            Ok(()) => ran.aborted,
        };
        if cancelled && still_there {
            return Err(Refusal::Declined);
        }
        if let Err(e) = &ran.result {
            return Err(failed(
                &format!(
                    "the shell did not move {} to the Recycle Bin",
                    dir.display()
                ),
                e,
            ));
        }
        if ran.aborted {
            return Err(Refusal::Failed(format!(
                "the shell stopped before moving {} to the Recycle Bin",
                dir.display()
            )));
        }
        if still_there {
            return Err(Refusal::Failed(format!(
                "{} is still there after the shell's move",
                dir.display()
            )));
        }
        let in_bin = !seen.done.is_empty()
            && seen.recycling > 0
            && seen.done.iter().all(|(hr, new)| hr.is_ok() && *new);
        if !in_bin {
            return Err(Refusal::Failed(format!(
                "{} is gone, but the shell did not report it in the Recycle Bin",
                dir.display()
            )));
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn seen(outright: u32, recycling: u32, done: Vec<(HRESULT, bool)>) -> Seen {
            Seen {
                outright,
                recycling,
                done,
            }
        }

        fn ok() -> Ran {
            Ran {
                result: Ok(()),
                aborted: false,
            }
        }

        #[test]
        fn the_verdict_takes_a_move_only_with_its_new_place_in_the_bin() {
            let gone = Path::new("Z:\\nm-recycle-verdict-not-there");
            let here = std::env::temp_dir();
            let fine = seen(0, 1, vec![(HRESULT(0), true)]);
            assert_eq!(verdict(gone, &fine, &ok()), Ok(()));
            // Vetoed and still there: the refusal that leaves the run.
            assert!(matches!(
                verdict(&here, &seen(1, 0, vec![]), &ok()),
                Err(Refusal::NotRecyclable(_))
            ));
            // A No in Windows' dialog, the run still there: declined, whichever way the shell
            // says it.
            for r in [
                Ran {
                    result: Err(COPYENGINE_E_USER_CANCELLED.into()),
                    aborted: true,
                },
                Ran {
                    result: Err(ERROR_CANCELLED.to_hresult().into()),
                    aborted: false,
                },
                Ran {
                    result: Ok(()),
                    aborted: true,
                },
            ] {
                assert_eq!(
                    verdict(&here, &seen(0, 0, vec![]), &r),
                    Err(Refusal::Declined)
                );
            }
            // Every other case is a failure, never a success.
            for (s, r, d) in [
                (seen(1, 0, vec![]), ok(), gone),
                (seen(0, 1, vec![(HRESULT(0), false)]), ok(), gone),
                (seen(0, 1, vec![(E_ABORT, true)]), ok(), gone),
                (seen(0, 0, vec![]), ok(), gone),
                (
                    seen(0, 1, vec![(HRESULT(0), true)]),
                    Ran {
                        result: Ok(()),
                        aborted: true,
                    },
                    gone,
                ),
                (
                    seen(0, 1, vec![(HRESULT(0), true)]),
                    Ran {
                        result: Err(E_ABORT.into()),
                        aborted: false,
                    },
                    gone,
                ),
                (fine, ok(), here.as_path()),
            ] {
                assert!(
                    matches!(verdict(d, &s, &r), Err(Refusal::Failed(_))),
                    "{}",
                    d.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    const MB: u64 = 1 << 20;

    fn fixed(volume: BinValues, global: BinValues) -> BinPolicy {
        BinPolicy {
            root: "B:\\".to_string(),
            has_bin: true,
            volume_mb: 488_353,
            volume,
            global,
            no_recycle_files: false,
        }
    }

    fn vals(nuke: Option<u32>, cap: Option<u32>) -> BinValues {
        BinValues {
            nuke_on_delete: nuke,
            max_capacity_mb: cap,
        }
    }

    #[test]
    fn a_bin_turned_off_is_refused_from_the_volume_or_the_global_key() {
        let none = BinValues::default();
        let off = bin_holds(&fixed(vals(Some(1), Some(50_000)), none.clone()), MB).unwrap_err();
        assert!(off.contains("turned off") && off.contains("B:\\"), "{off}");
        // The volume says nothing: the global key's 1 holds.
        assert!(bin_holds(&fixed(none.clone(), vals(Some(1), None)), MB).is_err());
        // The volume's own 0 wins over the global key.
        assert_eq!(
            bin_holds(&fixed(vals(Some(0), None), vals(Some(1), None)), MB),
            Ok(())
        );
        // The group policy turns every bin off.
        let mut gpo = fixed(vals(Some(0), Some(50_000)), none);
        gpo.no_recycle_files = true;
        assert!(bin_holds(&gpo, MB).unwrap_err().contains("turned off"));
    }

    #[test]
    fn a_run_larger_than_the_bin_is_refused_at_one_byte_over() {
        let p = fixed(vals(Some(0), Some(10)), BinValues::default());
        assert_eq!(bin_holds(&p, 10 * MB), Ok(()));
        let over = bin_holds(&p, 10 * MB + 1).unwrap_err();
        assert!(over.contains("11 MB, more than the 10 MB"), "{over}");
        // The volume says nothing: the global key's size holds.
        let g = fixed(BinValues::default(), vals(None, Some(10)));
        assert!(bin_holds(&g, 10 * MB + 1).is_err());
    }

    #[test]
    fn missing_keys_fall_to_windows_own_defaults() {
        // Windows' own default, as it wrote it into this machine's keys.
        assert_eq!(default_capacity_mb(306_412), 17_368);
        assert_eq!(default_capacity_mb(3_815_413), 192_818);
        assert_eq!(default_capacity_mb(488_353), 26_465);
        assert_eq!(default_capacity_mb(20_000), 2_000);
        let p = fixed(BinValues::default(), BinValues::default());
        assert_eq!(bin_holds(&p, 26_465 * MB), Ok(()), "on by default");
        assert!(bin_holds(&p, 26_465 * MB + 1).is_err());
    }

    #[test]
    fn a_drive_with_no_bin_is_refused_whatever_its_keys_say() {
        let p = BinPolicy {
            root: "\\\\server\\share\\".to_string(),
            has_bin: false,
            volume: vals(Some(0), Some(u32::MAX)),
            ..BinPolicy::default()
        };
        let why = bin_holds(&p, 1).unwrap_err();
        assert!(why.contains("has no Recycle Bin"), "{why}");
    }

    #[test]
    fn a_fake_policy_can_add_a_refusal_never_lift_one() {
        let dir = std::env::temp_dir().join(format!("nm-recycle-guard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("solve")).unwrap();
        std::fs::write(
            dir.join("solve").join("out.gabe"),
            vec![1u8; 2 * MB as usize],
        )
        .unwrap();
        let fine = || Ok(fixed(vals(Some(0), Some(100)), BinValues::default()));
        let small: BinPolicy =
            serde_json::from_str(r#"{"has_bin":true,"volume":{"MaxCapacity":1}}"#).unwrap();
        assert_eq!(guard(&dir, |_| fine(), None), Ok(()));
        assert!(matches!(
            guard(&dir, |_| fine(), Some(Ok(small))),
            Err(Refusal::NotRecyclable(w)) if w.contains("2 MB, more than the 1 MB")
        ));
        // The real drive refuses: a fake that would allow it does not lift that.
        let off = || Ok(fixed(vals(Some(1), None), BinValues::default()));
        assert!(matches!(
            guard(&dir, |_| off(), Some(fine())),
            Err(Refusal::NotRecyclable(_))
        ));
        // Settings that do not read are a failure, never taken as a bin that holds the run.
        assert!(matches!(
            guard(&dir, |_| Err("no".into()), None),
            Err(Refusal::Failed(_))
        ));
        assert!(matches!(
            guard(&dir, |_| fine(), Some(Err("bad file".into()))),
            Err(Refusal::Failed(_))
        ));
        assert!(serde_json::from_str::<BinPolicy>(r#"{"has_bin":true,"Nuke":1}"#).is_err());
        assert_eq!(
            std::fs::read(dir.join("solve").join("out.gabe"))
                .unwrap()
                .len(),
            2 * MB as usize,
            "the guard touches nothing"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn the_real_reader_finds_this_drives_bin_and_none_through_a_share() {
        let here = std::env::temp_dir();
        let p = imp::policy_of(&here).unwrap();
        assert!(p.has_bin && p.volume_mb > 0, "{p:?}");
        if let Some(unc) = through_share(&here) {
            let p = imp::policy_of(&unc).unwrap();
            assert!(!p.has_bin, "{p:?}");
        }
    }

    /// `dir` (on a lettered drive) as the same folder through the drive's administrative share,
    /// `\\localhost\X$\...`: a network path, which has no Recycle Bin, so the shell would delete it
    /// outright. `None` when the share is not reachable here.
    fn through_share(dir: &Path) -> Option<std::path::PathBuf> {
        let s = dir.to_str()?;
        let (drive, rest) = s.split_once(":\\")?;
        let unc = std::path::PathBuf::from(format!("\\\\localhost\\{drive}$\\{rest}"));
        unc.is_dir().then_some(unc)
    }

    #[test]
    fn a_folder_the_shell_cannot_recycle_is_refused_and_left_whole() {
        let dir = std::env::temp_dir().join(format!("nm-recycle-refuse-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let run = dir.join("20260101-000001-000-spps");
        std::fs::create_dir_all(run.join("solve")).unwrap();
        std::fs::write(run.join("run.json"), "{}").unwrap();
        std::fs::write(run.join("solve").join("out.gabe"), [7u8; 4096]).unwrap();
        let Some(unc) = through_share(&run) else {
            std::fs::remove_dir_all(&dir).unwrap();
            panic!(
                "\\\\localhost\\<drive>$ is not reachable here, so the refusal cannot be shown; \
                 this test needs the drive's administrative share"
            );
        };
        // Refused by guard 1, before the shell is asked: a network path keeps no bin.
        match to_recycle_bin(&unc) {
            Err(Refusal::NotRecyclable(why)) => {
                assert!(why.contains("has no Recycle Bin"), "{why}")
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(std::fs::read_to_string(run.join("run.json")).unwrap(), "{}");
        assert_eq!(
            std::fs::read(run.join("solve").join("out.gabe")).unwrap(),
            [7u8; 4096],
            "the run intact, every file"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Takes the folder that was at `path` back out of the Recycle Bin (the shell's own restore),
    /// so the test leaves nothing there.
    const RESTORE_PS: &str = "$ErrorActionPreference = 'Stop'; \
$p = $env:NM_RESTORE; $parent = Split-Path -Parent $p; $leaf = Split-Path -Leaf $p; \
$sh = New-Object -ComObject Shell.Application; $bin = $sh.Namespace(10); \
$hits = @($bin.Items() | Where-Object { $_.ExtendedProperty('System.Recycle.DeletedFrom') -eq $parent -and $_.Name -eq $leaf }); \
if ($hits.Count -ne 1) { [Console]::Error.WriteLine(\"$($hits.Count) items in the bin for $p\"); exit 2 }; \
$sh.Namespace($parent).MoveHere($hits[0], 0x14); \
for ($i = 0; $i -lt 100 -and -not (Test-Path -LiteralPath $p); $i++) { Start-Sleep -Milliseconds 100 }; \
if (-not (Test-Path -LiteralPath $p)) { exit 3 }";

    /// Puts a scratch folder in this user's Recycle Bin and takes it back out: run by hand
    /// (`--ignored`), since it touches the bin.
    #[test]
    #[ignore = "moves a scratch folder through this user's Recycle Bin and back"]
    fn a_folder_goes_to_the_recycle_bin_and_is_restored_from_it() {
        let dir = std::env::temp_dir().join(format!("nm-recycle-ok-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let run = dir.join("20260101-000001-000-spps");
        std::fs::create_dir_all(run.join("solve")).unwrap();
        std::fs::write(run.join("solve").join("out.gabe"), [9u8; 2048]).unwrap();
        assert_eq!(to_recycle_bin(&run), Ok(()));
        assert!(!run.exists(), "gone from where it was");
        let out = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", RESTORE_PS])
            .env("NM_RESTORE", &run)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "restore: {:?} {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            std::fs::read(run.join("solve").join("out.gabe")).unwrap(),
            [9u8; 2048],
            "restored whole from the Recycle Bin"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Answers No (`IDCANCEL`, the dialog's No) in the first window this process puts up, and
    /// prints what the window said.
    const ANSWER_NO_PS: &str = "Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes; \
Add-Type -Namespace W -Name U -MemberDefinition '[DllImport(\"user32.dll\")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);'; \
$A = [System.Windows.Automation.AutomationElement]; $S = [System.Windows.Automation.TreeScope]; \
$cond = New-Object System.Windows.Automation.PropertyCondition($A::ProcessIdProperty, [int]$env:NM_PID); \
for ($i = 0; $i -lt 300; $i++) { \
  $w = $A::RootElement.FindFirst($S::Children, $cond); \
  if ($w) { \
    $t = @($w.FindAll($S::Descendants, [System.Windows.Automation.Condition]::TrueCondition) | ForEach-Object { $_.Current.Name } | Where-Object { $_ }) -join ' | '; \
    [Console]::Out.WriteLine(\"$($w.Current.Name): $t\"); \
    [void][W.U]::PostMessage([IntPtr]$w.Current.NativeWindowHandle, 0x111, [IntPtr]2, [IntPtr]0); exit 0 }; \
  Start-Sleep -Milliseconds 100 }; exit 4";

    /// Guard 2 alone: guard 1 skipped on a path Windows cannot recycle (the drive's admin share),
    /// so the shell is asked to delete what it can only delete outright. It puts its own
    /// "permanently delete?" dialog (never a silent delete), a script answers No, and the answer
    /// is a refusal with the run whole. Run by hand (`--ignored`): it puts a dialog on screen.
    #[test]
    #[ignore = "puts Windows' delete dialog on screen and answers it No"]
    fn without_guard_one_windows_asks_and_a_no_is_a_refusal() {
        let dir = std::env::temp_dir().join(format!("nm-recycle-ask-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let run = dir.join("20260101-000001-000-spps");
        std::fs::create_dir_all(&run).unwrap();
        std::fs::write(run.join("run.json"), "{}").unwrap();
        let unc = through_share(&run).expect("the drive's administrative share");
        let helper = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", ANSWER_NO_PS])
            .env("NM_PID", std::process::id().to_string())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let r = std::thread::spawn(move || imp::recycle(&unc))
            .join()
            .unwrap();
        let said = helper.wait_with_output().unwrap();
        eprintln!(
            "the dialog: {}",
            String::from_utf8_lossy(&said.stdout).trim()
        );
        assert!(said.status.success(), "no dialog came up");
        assert_eq!(r, Err(Refusal::Declined));
        assert_eq!(std::fs::read_to_string(run.join("run.json")).unwrap(), "{}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn what_is_not_a_folder_is_refused() {
        let missing = std::env::temp_dir().join("nm-recycle-not-there-at-all");
        assert!(matches!(to_recycle_bin(&missing), Err(Refusal::Failed(_))));
    }
}
