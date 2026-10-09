//! R3's delete: a folder moved to the Recycle Bin, or nothing done at all.
//!
//! The shell's own file operation (`IFileOperation`) does the move, asked to recycle
//! (`FOF_ALLOWUNDO`, `FOFX_RECYCLEONDELETE`). Asked to recycle, the shell still deletes outright,
//! without asking when it is told not to ask, whatever it cannot recycle: a folder larger than
//! the drive's Recycle Bin may hold, a drive whose Recycle Bin is turned off, a network or
//! removable drive. So a progress sink watches every item before the shell touches it: an item
//! that the shell is not going to recycle (`PreDeleteItem` without
//! `TSF_DELETE_RECYCLE_IF_POSSIBLE`) aborts the whole operation before anything is deleted.
//! Afterwards the move counts only when the shell reported no error and nothing aborted, the
//! folder is gone, and the shell named the item it made in the Recycle Bin (`PostDeleteItem`'s
//! new item); anything else is said, never taken as done.
//!
//! The shell asks nothing (`FOF_NO_UI`): the Runs tab has already asked, and the one question
//! the shell would add, "delete it permanently?", is never put, because the sink refuses that
//! case before the shell can act on it.

use std::path::Path;

/// Why a folder was not moved to the Recycle Bin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The shell would have deleted it outright (no Recycle Bin for it); nothing was deleted.
    NotRecyclable,
    /// The move failed or was not confirmed; the sentence says what was seen.
    Failed(String),
}

/// Moves the folder `dir` (absolute) to the Recycle Bin, or refuses and leaves it in place.
pub fn to_recycle_bin(dir: &Path) -> Result<(), Refusal> {
    if !dir.is_dir() {
        return Err(Refusal::Failed(format!(
            "{} is not a folder",
            dir.display()
        )));
    }
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
    use super::Refusal;

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

    use windows::Win32::Foundation::E_ABORT;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
        CoInitializeEx, CoUninitialize,
    };
    use windows::Win32::UI::Shell::{
        FOF_ALLOWUNDO, FOF_NO_UI, FOFX_EARLYFAILURE, FOFX_RECYCLEONDELETE, FileOperation,
        IFileOperation, IFileOperationProgressSink, IFileOperationProgressSink_Impl, IShellItem,
        SHCreateItemFromParsingName, TSF_DELETE_RECYCLE_IF_POSSIBLE,
    };
    use windows::core::{HRESULT, HSTRING, PCWSTR, Ref, implement};

    use super::Refusal;

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
        /// The one veto: an item the shell is not going to recycle stops the operation before
        /// it is touched, so no item is ever deleted outright.
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
            op.SetOperationFlags(
                FOF_ALLOWUNDO | FOF_NO_UI | FOFX_RECYCLEONDELETE | FOFX_EARLYFAILURE,
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
                Err(Refusal::NotRecyclable)
            } else {
                // Not expected: the veto comes before the shell acts. Said, never hidden.
                Err(Refusal::Failed(format!(
                    "the shell would not recycle {}, and it is no longer there",
                    dir.display()
                )))
            };
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
            assert_eq!(
                verdict(&here, &seen(1, 0, vec![]), &ok()),
                Err(Refusal::NotRecyclable)
            );
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;

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
        assert_eq!(to_recycle_bin(&unc), Err(Refusal::NotRecyclable));
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

    #[test]
    fn what_is_not_a_folder_is_refused() {
        let missing = std::env::temp_dir().join("nm-recycle-not-there-at-all");
        assert!(matches!(to_recycle_bin(&missing), Err(Refusal::Failed(_))));
    }
}
