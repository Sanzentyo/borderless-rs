//! Legacy raw Win32 GUI removed.
//!
//! The crate now uses `windows-reactor`, which is backed by WinUI.  This module
//! is intentionally kept as a tiny compatibility shim so old internal paths that
//! referenced `gui::run` can be redirected to the new library entry point.

pub use crate::app::run;
