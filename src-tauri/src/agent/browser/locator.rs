use std::path::PathBuf;

pub fn default_candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    #[cfg(target_os = "macos")]
    {
        v.push(PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"));
        v.push(PathBuf::from("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"));
        v.push(PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium"));
    }
    #[cfg(target_os = "windows")]
    {
        v.push(PathBuf::from("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"));
        v.push(PathBuf::from("C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe"));
        v.push(PathBuf::from("C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe"));
        v.push(PathBuf::from("C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe"));
    }
    #[cfg(target_os = "linux")]
    {
        v.push(PathBuf::from("/usr/bin/google-chrome"));
        v.push(PathBuf::from("/usr/bin/google-chrome-stable"));
        v.push(PathBuf::from("/usr/bin/chromium"));
        v.push(PathBuf::from("/usr/bin/chromium-browser"));
        v.push(PathBuf::from("/usr/bin/microsoft-edge"));
    }
    v
}

pub fn locate(browser_path: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = browser_path {
        let p = PathBuf::from(p);
        return if p.is_file() { Some(p) } else { None };
    }
    default_candidates().into_iter().find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_path_wins_when_exists() {
        let dir = std::env::temp_dir().join(format!("cw-locator-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("fake-browser");
        std::fs::write(&exe, "#!/bin/sh").unwrap();
        assert_eq!(locate(Some(exe.to_str().unwrap())), Some(exe.clone()));
        assert!(locate(Some(dir.join("missing").to_str().unwrap())).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn candidates_are_non_empty() {
        assert!(!default_candidates().is_empty());
    }

    #[test]
    fn locate_without_settings_returns_existing_file_or_none() {
        let found = locate(None);
        if found.is_some() {
            assert!(found.unwrap().is_file());
        }
    }
}
