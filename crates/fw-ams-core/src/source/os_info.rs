//! Operating system name and version for the System page (and bug reports).

/// e.g. "Windows 11 Pro 25H2 (Build 26200.7171)" or "Arch Linux · Kernel 6.17.1-arch1-1"
pub fn os_version() -> Option<String> {
    #[cfg(windows)]
    {
        windows::version()
    }
    #[cfg(target_os = "linux")]
    {
        let release = std::fs::read_to_string("/etc/os-release")
            .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
            .ok();
        let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").ok();
        linux_label(release.as_deref(), kernel.as_deref())
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}

/// Builds the Windows label from the registry values under `CurrentVersion`.
/// `ProductName` still says "Windows 10" on Windows 11, so the build number decides.
pub fn windows_label(product: Option<&str>, display: Option<&str>, build: Option<u32>, ubr: Option<u32>) -> Option<String> {
    let mut name = product?.trim().to_owned();
    if build.is_some_and(|b| b >= 22000) && name.starts_with("Windows 10") {
        name = name.replacen("Windows 10", "Windows 11", 1);
    }
    if let Some(d) = display.map(str::trim).filter(|d| !d.is_empty()) {
        name.push(' ');
        name.push_str(d);
    }
    match (build, ubr) {
        (Some(b), Some(u)) => name.push_str(&format!(" (Build {b}.{u})")),
        (Some(b), None) => name.push_str(&format!(" (Build {b})")),
        _ => {}
    }
    Some(name)
}

/// `PRETTY_NAME` from os-release plus the kernel release
pub fn linux_label(os_release: Option<&str>, kernel: Option<&str>) -> Option<String> {
    let pretty = os_release.and_then(|r| {
        r.lines().find_map(|l| {
            let v = l.strip_prefix("PRETTY_NAME=")?.trim();
            Some(v.trim_matches(|c| c == '"' || c == '\'').to_owned())
        })
    });
    let kernel = kernel.map(str::trim).filter(|k| !k.is_empty());
    match (pretty.filter(|p| !p.is_empty()), kernel) {
        (Some(p), Some(k)) => Some(format!("{p} · Kernel {k}")),
        (Some(p), None) => Some(p),
        (None, Some(k)) => Some(format!("Linux · Kernel {k}")),
        (None, None) => None,
    }
}

#[cfg(windows)]
mod windows {
    use windows_sys::Win32::System::Registry::{
        RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
    };

    const KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn string(name: &str) -> Option<String> {
        let (key, name) = (wide(KEY), wide(name));
        let mut buf = [0u16; 256];
        let mut len = std::mem::size_of_val(&buf) as u32;
        // SAFETY: buffer and its byte length are valid for the call
        let r = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut len,
            )
        };
        if r != 0 {
            return None;
        }
        let chars = (len as usize / 2).min(buf.len());
        let s = String::from_utf16_lossy(&buf[..chars]);
        Some(s.trim_end_matches('\0').to_owned())
    }

    fn dword(name: &str) -> Option<u32> {
        let (key, name) = (wide(KEY), wide(name));
        let mut v = 0u32;
        let mut len = 4u32;
        // SAFETY: v is a valid 4-byte buffer
        let r = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&mut v as *mut u32).cast(),
                &mut len,
            )
        };
        (r == 0).then_some(v)
    }

    pub fn version() -> Option<String> {
        let build = string("CurrentBuildNumber").and_then(|b| b.parse().ok());
        super::windows_label(
            string("ProductName").as_deref(),
            string("DisplayVersion").as_deref(),
            build,
            dword("UBR"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_11_from_build_number() {
        assert_eq!(
            windows_label(Some("Windows 10 Pro"), Some("25H2"), Some(26200), Some(7171)).as_deref(),
            Some("Windows 11 Pro 25H2 (Build 26200.7171)")
        );
        assert_eq!(
            windows_label(Some("Windows 10 Home"), Some("22H2"), Some(19045), None).as_deref(),
            Some("Windows 10 Home 22H2 (Build 19045)")
        );
        assert_eq!(windows_label(None, Some("25H2"), Some(26200), None), None);
    }

    #[test]
    fn linux_pretty_name_and_kernel() {
        let release = "NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\nID=arch\n";
        assert_eq!(
            linux_label(Some(release), Some("6.17.1-arch1-1\n")).as_deref(),
            Some("Arch Linux · Kernel 6.17.1-arch1-1")
        );
        assert_eq!(linux_label(None, Some("6.1.0")).as_deref(), Some("Linux · Kernel 6.1.0"));
        assert_eq!(linux_label(None, None), None);
    }
}
