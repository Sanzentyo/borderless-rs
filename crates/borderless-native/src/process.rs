use crate::ffi;
use borderless_core::{Pid, ProcessName};
use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};

pub(crate) fn process_names() -> Vec<(Pid, ProcessName)> {
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return Vec::new();
    };
    if snapshot == INVALID_HANDLE_VALUE {
        return Vec::new();
    }

    let dw_size = u32::try_from(std::mem::size_of::<PROCESSENTRY32W>())
        .expect("PROCESSENTRY32W size fits in u32");
    let mut entry = PROCESSENTRY32W {
        dwSize: dw_size,
        ..Default::default()
    };
    let mut out = Vec::new();
    let mut ok = unsafe { Process32FirstW(snapshot, &raw mut entry).is_ok() };
    while ok {
        if let (Some(pid), Ok(name)) = (
            Pid::new(entry.th32ProcessID),
            ProcessName::new(ffi::pwstr_to_string(&entry.szExeFile)),
        ) {
            out.push((pid, name));
        }
        ok = unsafe { Process32NextW(snapshot, &raw mut entry).is_ok() };
    }
    unsafe {
        let _ = CloseHandle(snapshot);
    }
    out
}
