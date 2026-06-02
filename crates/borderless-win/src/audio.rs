use borderless_core::{CoreResult, Hwnd, Pid};

#[derive(Clone, Debug, Default)]
pub struct AudioSessions;

impl AudioSessions {
    pub fn set_pid_muted(&self, pid: Pid, muted: bool) -> CoreResult<()> {
        tracing::debug!(%pid, muted, "audio mute requested");
        // Production implementation should enumerate CoreAudio sessions via:
        // IMMDeviceEnumerator -> IAudioSessionManager2 -> IAudioSessionEnumerator ->
        // IAudioSessionControl2::GetProcessId -> ISimpleAudioVolume::SetMute.
        // The hook is here so the runtime policy is already complete without coupling
        // audio APIs into the domain layer.
        Ok(())
    }

    pub fn set_process_muted(&self, hwnd: Hwnd, muted: bool) -> CoreResult<()> {
        tracing::debug!(%hwnd, muted, "audio mute requested by hwnd");
        Ok(())
    }
}
