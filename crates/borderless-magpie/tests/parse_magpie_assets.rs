// SPDX-License-Identifier: GPL-3.0-or-later

use borderless_magpie::parse_magpiefx_file;
use std::path::{Path, PathBuf};

#[test]
#[ignore = "set MAGPIE_EFFECTS_DIR to a Magpie src/Effects checkout to verify external assets"]
fn parses_magpie_effect_assets() {
    let Some(root) = std::env::var_os("MAGPIE_EFFECTS_DIR").map(PathBuf::from) else {
        return;
    };

    let files = collect_hlsl_files(&root).expect("failed to enumerate Magpie effect files");
    assert!(
        !files.is_empty(),
        "MAGPIE_EFFECTS_DIR did not contain any .hlsl files: {}",
        root.display()
    );

    let failures = files
        .iter()
        .filter_map(|path| {
            parse_magpiefx_file(path)
                .err()
                .map(|err| format!("{}: {err}", path.display()))
        })
        .collect::<Vec<_>>();

    assert!(
        failures.is_empty(),
        "failed to parse {} Magpie effect files:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn collect_hlsl_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_hlsl_files_into(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_hlsl_files_into(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_hlsl_files_into(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "hlsl")
        {
            files.push(path);
        }
    }
    Ok(())
}
