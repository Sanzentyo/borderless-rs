use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

#[derive(Debug)]
pub struct ProfileSpan {
    label: &'static str,
    start: Instant,
    enabled: bool,
}

impl ProfileSpan {
    #[must_use]
    pub fn start(label: &'static str) -> Self {
        let enabled = enabled();
        if enabled {
            mark(format!("{label}: start"));
        }
        Self {
            label,
            start: Instant::now(),
            enabled,
        }
    }

    pub fn mark(message: impl AsRef<str>) {
        if enabled() {
            mark(message);
        }
    }
}

impl Drop for ProfileSpan {
    fn drop(&mut self) {
        if self.enabled {
            mark(format!(
                "{}: done in {:?}",
                self.label,
                self.start.elapsed()
            ));
        }
    }
}

fn enabled() -> bool {
    std::env::var("BORDERLESS_PROFILE").is_ok_and(|value| !value.is_empty() && value != "0")
}

fn mark(message: impl AsRef<str>) {
    let start = *START.get_or_init(Instant::now);
    let line = format!(
        "[borderless-profile +{:?}] {}",
        start.elapsed(),
        message.as_ref()
    );
    eprintln!("{line}");
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(profile_log_path())
    {
        let _ = writeln!(file, "{line}");
    }
}

fn profile_log_path() -> PathBuf {
    std::env::temp_dir().join("borderless-oxide-profile.log")
}
