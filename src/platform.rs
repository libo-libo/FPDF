use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub fn config_base() -> Option<OsString> {
    #[cfg(windows)]
    {
        return std::env::var_os("APPDATA").filter(|value| !value.is_empty());
    }
    #[cfg(not(windows))]
    {
        if let Some(value) = std::env::var_os("XDG_CONFIG_HOME")
            && !value.is_empty()
            && Path::new(&value).is_absolute()
        {
            return Some(value);
        }
        std::env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .map(|home| PathBuf::from(home).join(".config").into_os_string())
    }
}

pub fn config_path(file: &str) -> Option<PathBuf> {
    config_path_from(config_base(), file)
}

pub fn config_path_from(base: Option<OsString>, file: &str) -> Option<PathBuf> {
    base.filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|base| base.join("fpdf").join(file))
}

pub fn font_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(directory) = exe.parent()
    {
        candidates.push(directory.join("fonts/wqy-microhei.ttc"));
        candidates.push(directory.join("runtime/fonts/wqy-microhei.ttc"));
    }
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/fonts/wqy-microhei.ttc"));
    #[cfg(windows)]
    candidates.extend([
        PathBuf::from("C:/Windows/Fonts/msyh.ttc"),
        PathBuf::from("C:/Windows/Fonts/simhei.ttf"),
    ]);
    #[cfg(not(windows))]
    candidates.extend([
        PathBuf::from("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
        PathBuf::from("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"),
        PathBuf::from("/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc"),
        PathBuf::from("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc"),
        PathBuf::from("/usr/share/fonts/wqy-microhei/wqy-microhei.ttc"),
        PathBuf::from("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc"),
        PathBuf::from("/usr/share/fonts/google-droid/DroidSansFallbackFull.ttf"),
    ]);
    candidates
}

pub fn node_executable(runtime: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) { "node.exe" } else { "node" };
    let bundled = runtime.join(name);
    if bundled.is_file() {
        return Some(bundled);
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(name))
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_paths_live_below_the_platform_user_directory() {
        let root = tempfile::tempdir().unwrap();
        let path = config_path_from(Some(root.path().as_os_str().to_owned()), "agent.txt").unwrap();
        assert_eq!(path, root.path().join("fpdf/agent.txt"));
        assert!(config_path_from(Some(Default::default()), "agent.txt").is_none());
    }
}
