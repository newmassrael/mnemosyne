//! The half of a sweep that has to survive the sweep's own death.
//!
//! WHAT WENT WRONG, measured rather than imagined. Round 1061 killed a running
//! harness and the `cargo test` it had started kept going: it stayed attached to
//! the same log file and wrote a second copy of seven doc-test targets into it,
//! so the re-run's control announced 159 targets under 152 names and the
//! name-uniqueness gate refused the sweep. That refusal is the only reason
//! anybody found out. The worse half was quieter — a harness killed between
//! `apply` and `restore` leaves the injection IN THE TREE, and nothing in the
//! process that died can put it back.
//!
//! Both are the same defect: the harness owned a tree and a process tree, and
//! owned neither through its own death.
//!
//! WHAT A PROCESS CAN AND CANNOT DO ABOUT ITS OWN DEATH. `SIGKILL` cannot be
//! caught, so a design where only the harness can clean up is a design that
//! cannot clean up. What the kernel does offer is a relationship: a child can
//! ask to be signalled when its parent dies (`PR_SET_PDEATHSIG`), and a process
//! group can be signalled as one. So the suite is not started by the harness
//! directly — it is started by a SUPERVISOR, one re-exec of this same binary,
//! which:
//!
//!   - leads its own process group, so signalling it never reaches back into the
//!     harness, and the harness can signal it and nothing else;
//!   - is told to receive `SIGTERM` when the harness dies BY ANY MEANS, which is
//!     the only notification a `SIGKILL`ed parent can arrange in advance;
//!   - starts the suite in a group of ITS own, so one `kill` reaches every test
//!     binary the suite spawned rather than only the `cargo` that spawned them;
//!   - holds the path to the originals, so the tree gets put back by whoever is
//!     still alive to do it;
//!   - and forwards the suite's death faithfully, re-raising the signal that
//!     killed it rather than translating it into an exit code, so a run that was
//!     killed is not read as a run that finished.
//!
//! The originals are on DISK rather than only in the harness's memory for the
//! same reason: the process that holds them may be the process that dies.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicI32, Ordering};

use serde::{Deserialize, Serialize};

/// One file as it was before any injection touched it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Original {
    /// The file in the tree under measurement.
    pub repo_file: PathBuf,
    /// The copy of its pre-sweep bytes.
    pub backup: PathBuf,
}

/// Every file this sweep may edit, where its pre-sweep bytes live, and who is
/// answerable for putting them back.
///
/// The owner is what tells the two reasons for finding an index apart. A sweep
/// whose pid is gone died holding the tree, and its originals are evidence. A
/// sweep whose pid is still there is RUNNING in this tree right now, and a
/// second sweep started under it would edit the same files, write the same logs,
/// and read the other's injection as its own baseline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Originals {
    pub owner: i32,
    pub files: Vec<Original>,
}

/// Whether the process that wrote an originals index is still there.
///
/// `pid > 1` because signal 0 to pid 0 asks about the CALLER's process group,
/// which would answer "alive" for every sweep that ever ran.
pub fn owner_alive(pid: i32) -> bool {
    pid > 1 && unsafe { libc::kill(pid, 0) == 0 }
}

/// One sweep's private working directory: the originals, and the copy of this
/// binary that supervises every run. Under the log directory because that is the
/// one place a manifest already names as this sweep's to write in.
fn sweep_dir(logs: &Path) -> PathBuf {
    logs.join("sweep")
}

/// Where a sweep keeps the originals it may have to be restored from by someone
/// else.
pub fn index_path(logs: &Path) -> PathBuf {
    sweep_dir(logs).join("originals.json")
}

/// The sweep's own copy of the binary running it.
///
/// A sweep can be aimed at the tree that BUILDS it — this crate's `self-check`
/// is exactly that — and then the suite replaces the binary this process is
/// executing. `/proc/self/exe` afterwards names a path that no longer exists,
/// and the next run cannot be started at all: the second injection of the first
/// self-check ever run died with `No such file or directory`. Re-resolving the
/// path would be worse than failing, because it would supervise the following
/// runs with whatever the suite just built — including the injected build.
///
/// So the supervisor is a COPY, taken before the first run: the code that
/// started the sweep is the code that owns it, whatever happens to the tree.
pub fn supervisor_path(logs: &Path) -> PathBuf {
    sweep_dir(logs).join("supervisor")
}

/// Take the sweep's copy of this binary.
pub fn copy_self(logs: &Path) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot find my own binary: {e}"))?;
    let target = supervisor_path(logs);
    let dir = sweep_dir(logs);
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    fs::copy(&exe, &target).map_err(|e| {
        format!(
            "cannot take a copy of {} to supervise with: {e}",
            exe.display()
        )
    })?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("{}: {e}", target.display()))?;
    Ok(target)
}

/// Write the pre-sweep bytes of every file the sweep may touch, and the index a
/// dying supervisor reads them back from.
pub fn write_originals(
    logs: &Path,
    snapshot: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<PathBuf, String> {
    let index = index_path(logs);
    let dir = index
        .parent()
        .ok_or_else(|| "the originals index has no directory".to_string())?
        .to_path_buf();
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut files = Vec::new();
    for (ordinal, (repo_file, bytes)) in snapshot.iter().enumerate() {
        // The ordinal, not the basename alone: two manifests may inject into
        // files that share a name in different directories, and a backup that
        // silently overwrote another one would restore the wrong bytes.
        let stem = repo_file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());
        let backup = dir.join(format!("{ordinal:03}-{stem}"));
        fs::write(&backup, bytes).map_err(|e| format!("{}: {e}", backup.display()))?;
        files.push(Original {
            repo_file: repo_file.clone(),
            backup,
        });
    }
    let originals = Originals {
        owner: std::process::id() as i32,
        files,
    };
    fs::write(
        &index,
        serde_json::to_string_pretty(&originals).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("{}: {e}", index.display()))?;
    Ok(index)
}

pub fn read_originals(index: &Path) -> Result<Originals, String> {
    let text =
        fs::read_to_string(index).map_err(|e| format!("{} unreadable: {e}", index.display()))?;
    serde_json::from_str(&text)
        .map_err(|e| format!("{} is not an originals index: {e}", index.display()))
}

/// The files that no longer hold their pre-sweep bytes — the tree's answer to
/// "did the sweep that died leave an injection behind".
pub fn still_injected(originals: &Originals) -> Result<Vec<PathBuf>, String> {
    let mut differing = Vec::new();
    for file in &originals.files {
        let was = fs::read(&file.backup)
            .map_err(|e| format!("{} unreadable: {e}", file.backup.display()))?;
        match fs::read(&file.repo_file) {
            Ok(now) if now == was => {}
            _ => differing.push(file.repo_file.clone()),
        }
    }
    Ok(differing)
}

/// Put every file back to its pre-sweep bytes, reading each one back to say so.
pub fn restore_originals(originals: &Originals) -> Result<(), String> {
    for file in &originals.files {
        let was = fs::read(&file.backup)
            .map_err(|e| format!("{} unreadable: {e}", file.backup.display()))?;
        fs::write(&file.repo_file, &was)
            .map_err(|e| format!("{} unwritable: {e}", file.repo_file.display()))?;
        let back = fs::read(&file.repo_file)
            .map_err(|e| format!("{} unreadable: {e}", file.repo_file.display()))?;
        if back != was {
            return Err(format!(
                "{} did not come back to what it was",
                file.repo_file.display()
            ));
        }
    }
    Ok(())
}

/// Drop the working directory of a sweep that finished under its own control.
///
/// Leaving it is how the NEXT sweep learns that this one died: the gate at
/// startup reads the originals, and a sweep that ended normally has nothing to
/// say. A supervisor still running out of the deleted copy is unaffected — it
/// holds the inode, not the name.
pub fn clear_originals(logs: &Path) -> Result<(), String> {
    let dir = sweep_dir(logs);
    if !dir.exists() {
        return Ok(());
    }
    fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))
}

/// The signals a person or a shell uses to stop a run. `SIGKILL` is not among
/// them because it cannot be — that is what the supervisor is for.
const INTERRUPTS: [i32; 3] = [libc::SIGINT, libc::SIGTERM, libc::SIGHUP];

/// Block the interrupt signals in every thread of this process, so the only
/// place they are received is the thread that asks for them.
///
/// Must run before any thread is spawned: the mask is inherited, and a thread
/// started before this call could take the signal out from under the handler.
pub fn block_interrupts() -> Result<(), String> {
    unsafe {
        let mut set = std::mem::zeroed::<libc::sigset_t>();
        libc::sigemptyset(&mut set);
        for signal in INTERRUPTS {
            libc::sigaddset(&mut set, signal);
        }
        if libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) != 0 {
            return Err(format!(
                "cannot take charge of the interrupt signals: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    Ok(())
}

/// Wait for one of the interrupt signals and say which arrived.
///
/// `sigwait` rather than a handler: a handler may only call async-signal-safe
/// functions, and everything this has to do when it wakes — kill a process
/// group, write files back, report — is none of those. A thread parked here is
/// an ordinary thread doing ordinary work.
pub fn wait_for_interrupt() -> i32 {
    unsafe {
        let mut set = std::mem::zeroed::<libc::sigset_t>();
        libc::sigemptyset(&mut set);
        for signal in INTERRUPTS {
            libc::sigaddset(&mut set, signal);
        }
        let mut received: i32 = 0;
        if libc::sigwait(&set, &mut received) != 0 {
            return libc::SIGTERM;
        }
        received
    }
}

/// The name of a signal, for a message a person reads.
pub fn signal_name(signal: i32) -> &'static str {
    match signal {
        libc::SIGINT => "SIGINT",
        libc::SIGTERM => "SIGTERM",
        libc::SIGHUP => "SIGHUP",
        libc::SIGKILL => "SIGKILL",
        _ => "a signal",
    }
}

/// Signal a whole process group — the suite AND every test binary it started.
pub fn kill_group(pgid: i32, signal: i32) {
    if pgid <= 0 {
        return;
    }
    unsafe {
        libc::kill(-pgid, signal);
    }
}

/// Start this command in a process group of its own, with the signal mask this
/// process is holding cleared.
///
/// The mask matters: interrupts are blocked here so one thread can own them, and
/// a blocked mask is INHERITED THROUGH EXEC. Without this the suite would start
/// deaf to the very signals a person uses to stop it.
fn in_its_own_group(command: &mut Command, die_with_parent: bool) {
    unsafe {
        command.pre_exec(move || {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if die_with_parent && libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            let mut set = std::mem::zeroed::<libc::sigset_t>();
            libc::sigemptyset(&mut set);
            for signal in INTERRUPTS {
                libc::sigaddset(&mut set, signal);
            }
            if libc::pthread_sigmask(libc::SIG_UNBLOCK, &set, std::ptr::null_mut()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// Wrap a command in a supervisor: `<the sweep's copy> --supervise <index|-> --
/// argv…`.
///
/// The supervisor is this same program because the alternative is a shell
/// script, and a shell script that silently does nothing is the failure mode
/// this whole tool exists to remove. It is the sweep's own COPY of it because
/// the suite may replace the original — see `supervisor_path`.
pub fn supervised_command(
    supervisor: &Path,
    originals_index: Option<&Path>,
    argv: &[String],
) -> Result<Command, String> {
    let mut command = Command::new(supervisor);
    command.arg("--supervise").arg(
        originals_index
            .map(|path| path.as_os_str().to_os_string())
            .unwrap_or_else(|| std::ffi::OsString::from("-")),
    );
    command.arg("--");
    command.args(argv);
    in_its_own_group(&mut command, true);
    Ok(command)
}

/// The `--supervise` half: run the suite, and be the one still standing.
///
/// Never returns — the whole point is that the status this process exits with IS
/// the suite's status, including death by signal.
pub fn supervise(originals_index: Option<PathBuf>, argv: &[String]) -> ! {
    if argv.is_empty() {
        eprintln!("injection-harness --supervise: no command to supervise");
        std::process::exit(2);
    }
    if let Err(problem) = block_interrupts() {
        eprintln!("injection-harness --supervise: {problem}");
        std::process::exit(2);
    }
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]);
    in_its_own_group(&mut command, false);
    let child = match command.spawn() {
        Ok(child) => child,
        Err(problem) => {
            eprintln!("injection-harness --supervise: {:?}: {problem}", argv);
            std::process::exit(2);
        }
    };
    // The child leads its own group, so its pid is that group's id and one
    // signal reaches every test binary underneath it.
    let suite_group = child.id() as i32;

    static INTERRUPTED: AtomicI32 = AtomicI32::new(0);
    std::thread::spawn(move || {
        let signal = wait_for_interrupt();
        INTERRUPTED.store(signal, Ordering::SeqCst);
        // SIGKILL to the suite: it is being abandoned, not asked to finish, and
        // a `cargo` that exits gracefully still leaves its test binaries running.
        kill_group(suite_group, libc::SIGKILL);
    });

    let mut child = child;
    let status = match child.wait() {
        Ok(status) => status,
        Err(problem) => {
            eprintln!("injection-harness --supervise: cannot wait for the suite: {problem}");
            std::process::exit(2);
        }
    };

    let interrupted = INTERRUPTED.load(Ordering::SeqCst);
    if interrupted != 0 {
        // THE TREE COMES BACK EVEN THOUGH THE HARNESS IS GONE. This is the whole
        // reason the originals are on disk and this process exists.
        if let Some(index) = originals_index {
            match read_originals(&index).and_then(|originals| restore_originals(&originals)) {
                Ok(()) => eprintln!(
                    "injection-harness --supervise: {} — suite killed, tree restored",
                    signal_name(interrupted)
                ),
                Err(problem) => eprintln!(
                    "injection-harness --supervise: {} — suite killed, TREE NOT RESTORED: {problem}",
                    signal_name(interrupted)
                ),
            }
        }
        die_by(interrupted);
    }

    match status.signal() {
        // The suite was killed. Die the same way, so the harness reads a run
        // that was killed rather than a run that exited.
        Some(signal) => die_by(signal),
        None => std::process::exit(status.code().unwrap_or(1)),
    }
}

/// Die of the given signal, so a waiting parent sees the death that happened.
fn die_by(signal: i32) -> ! {
    unsafe {
        libc::signal(signal as libc::c_int, libc::SIG_DFL);
        let mut set = std::mem::zeroed::<libc::sigset_t>();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, signal);
        libc::pthread_sigmask(libc::SIG_UNBLOCK, &set, std::ptr::null_mut());
        libc::raise(signal);
    }
    // Only reached if the signal was somehow ignored; still not an exit 0.
    std::process::exit(128 + signal);
}
