use borderless_core::{
    Favorite, FavoriteOptions, Hwnd, MonitorSnapshot, Pid, ProcessName, WindowSnapshot, WindowTitle,
};
use ractor::RpcReplyPort;

#[derive(Debug)]
pub enum ControllerMsg {
    ListWindows(RpcReplyPort<Vec<WindowSnapshot>>),
    ListMonitors(RpcReplyPort<Vec<MonitorSnapshot>>),
    ApplyWindow(WindowSnapshot, RpcReplyPort<Result<Hwnd, String>>),
    ApplyByHwnd(Hwnd, RpcReplyPort<Result<Hwnd, String>>),
    ApplyByHwndWithOptions(Hwnd, FavoriteOptions, RpcReplyPort<Result<Hwnd, String>>),
    ApplyByPid(Pid, RpcReplyPort<Result<Hwnd, String>>),
    ApplyByProcessName(ProcessName, RpcReplyPort<Result<Hwnd, String>>),
    ApplyByTitle(WindowTitle, RpcReplyPort<Result<Hwnd, String>>),
    Restore(Hwnd, RpcReplyPort<Result<(), String>>),
    AddFavorite(Favorite, RpcReplyPort<Result<(), String>>),
    RemoveFavorite(String, RpcReplyPort<Result<(), String>>),
    SaveConfig(RpcReplyPort<Result<(), String>>),
    SetTaskbarVisible(bool, RpcReplyPort<Result<(), String>>),
    SetCursorVisible(bool, RpcReplyPort<Result<(), String>>),
}

#[derive(Debug, Clone)]
pub enum WatcherMsg {
    Tick,
    Stop,
}

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    WindowsChanged(Vec<WindowSnapshot>),
    WindowApplied(Hwnd),
    WindowRestored(Hwnd),
    FavoriteMatched(String, Hwnd),
    Error(String),
}
