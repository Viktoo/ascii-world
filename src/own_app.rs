//! On a Mac, the game played from a terminal is its own app to the system:
//! asked for the microphone (or the local network), the system names the
//! game and gives its reason (the `Info.plist` that `build.rs` puts in the
//! binary), not the terminal. The game starts itself again with the
//! terminal disclaimed as responsible for it, and this one waits for it.
//!
//! Release builds only, run outside the app bundle (the bundle is its own
//! app already): the system asks again whenever the binary changes, which
//! for a dev build is every build.

#[cfg(target_os = "macos")]
pub fn become_own_app() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    const MARK: &str = "POCKET_OWN_APP";
    if crate::db::RELEASE == "dev" || std::env::var_os(MARK).is_some() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    if exe.to_string_lossy().contains(".app/Contents/MacOS/") {
        return;
    }
    unsafe extern "C" {
        // libsystem's, unexported in headers; what Xcode and Qt Creator use.
        fn responsibility_spawnattrs_setdisclaim(attr: *mut libc::posix_spawnattr_t, disclaim: libc::c_int) -> libc::c_int;
    }
    let Ok(path) = CString::new(exe.as_os_str().as_bytes()) else { return };
    let mut args: Vec<CString> = vec![path.clone()];
    args.extend(std::env::args_os().skip(1).filter_map(|a| CString::new(a.as_bytes()).ok()));
    let mut argv: Vec<*mut libc::c_char> = args.iter().map(|a| a.as_ptr() as *mut _).collect();
    argv.push(std::ptr::null_mut());
    // SAFETY: nothing else runs yet (no other threads read the environment);
    // the attributes live until the spawn is done.
    unsafe {
        std::env::set_var(MARK, "1");
        let mut attr: libc::posix_spawnattr_t = std::ptr::null_mut();
        let mut pid: libc::pid_t = 0;
        let ok = libc::posix_spawnattr_init(&mut attr) == 0
            && responsibility_spawnattrs_setdisclaim(&mut attr, 1) == 0
            && libc::posix_spawn(&mut pid, path.as_ptr(), std::ptr::null(), &attr, argv.as_ptr(), *libc::_NSGetEnviron()) == 0;
        libc::posix_spawnattr_destroy(&mut attr);
        if !ok {
            // Played here instead, as before.
            std::env::remove_var(MARK);
            crate::log::info("own app: couldn't start the game on its own; the terminal answers for it");
            return;
        }
        // Ctrl-C reaches both; the game decides when it's done.
        libc::signal(libc::SIGINT, libc::SIG_IGN);
        let mut status = 0;
        while libc::waitpid(pid, &mut status, 0) < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {}
        let code = if libc::WIFEXITED(status) { libc::WEXITSTATUS(status) } else { 128 + libc::WTERMSIG(status) };
        std::process::exit(code);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn become_own_app() {}
