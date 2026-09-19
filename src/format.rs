use std::path::{Component, Path};

/// Render `target` as seen from `base`, walking up with `..` when that is
/// shorter than naming the whole path. Falls back to the absolute path when
/// the two share no common root.
pub fn relative_path(base: &Path, target: &Path) -> String {
    let parts = |p: &Path| -> Vec<String> {
        p.components()
            .filter_map(|c| match c {
                Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect()
    };
    if !base.is_absolute() || !target.is_absolute() {
        return target.to_string_lossy().into_owned();
    }
    let (base, target) = (parts(base), parts(target));
    let shared = base.iter().zip(&target).take_while(|(a, b)| a == b).count();
    if shared == 0 {
        return format!("/{}", target.join("/"));
    }
    let mut out: Vec<&str> = vec![".."; base.len() - shared];
    out.extend(target[shared..].iter().map(String::as_str));
    if out.is_empty() {
        ".".to_string()
    } else {
        out.join("/")
    }
}

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
const MONTH: i64 = 30 * DAY;
const YEAR: i64 = 365 * DAY;

/// How long ago a timestamp was, in the shape git uses: "3 days ago".
pub fn relative_time(timestamp: i64, now: i64) -> String {
    let elapsed = now - timestamp;
    if elapsed < 0 {
        return "in the future".to_string();
    }
    let (count, unit) = match elapsed {
        e if e < MINUTE => return "just now".to_string(),
        e if e < HOUR => (e / MINUTE, "minute"),
        e if e < DAY => (e / HOUR, "hour"),
        e if e < MONTH => (e / DAY, "day"),
        e if e < YEAR => (e / MONTH, "month"),
        e => (e / YEAR, "year"),
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_paths_under_the_current_directory() {
        assert_eq!(
            relative_path(Path::new("/a/b"), Path::new("/a/b/c/d.rs")),
            "c/d.rs"
        );
    }

    #[test]
    fn walks_up_when_the_path_is_elsewhere() {
        assert_eq!(
            relative_path(Path::new("/a/b/c"), Path::new("/a/b/x.rs")),
            "../x.rs"
        );
    }

    #[test]
    fn keeps_unrelated_paths_absolute() {
        assert_eq!(relative_path(Path::new("/a"), Path::new("/x/y")), "/x/y");
    }

    #[test]
    fn describes_elapsed_time_the_way_git_does() {
        assert_eq!(relative_time(0, 30), "just now");
        assert_eq!(relative_time(0, 60), "1 minute ago");
        assert_eq!(relative_time(0, 3 * HOUR), "3 hours ago");
        assert_eq!(relative_time(0, 2 * DAY), "2 days ago");
        assert_eq!(relative_time(0, 5 * YEAR), "5 years ago");
    }
}
