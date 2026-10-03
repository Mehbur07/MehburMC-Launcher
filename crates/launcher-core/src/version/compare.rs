//! Minecraft version ordering across both numbering schemes:
//! `1.8.9 < 1.21.11 < 26.1 < 26.3`, with pre-release suffixes ordered
//! `snapshot < pre < rc < release`.
//!
//! The release time in the version manifest is the authoritative order for
//! lists; this comparison is for rules like "is this at least 1.13?" where
//! only ids are at hand.

use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Parsed {
    nums: Vec<u32>,
    /// `(kind rank, number)`; `None` means a final release.
    pre: Option<(u8, u32)>,
}

fn parse(id: &str) -> Option<Parsed> {
    let id = id.trim();
    // Split numeric core from a suffix such as `-pre1`, `-rc-3`, `-snapshot-2`, ` Pre-Release 2`.
    let core_end = id
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(id.len());
    let (core, rest) = id.split_at(core_end);
    if core.is_empty() || core.ends_with('.') {
        return None;
    }
    let nums = core
        .split('.')
        .map(|p| p.parse::<u32>().ok())
        .collect::<Option<Vec<_>>>()?;

    let rest = rest
        .trim_start_matches(['-', ' ', '_'])
        .to_ascii_lowercase();
    if rest.is_empty() {
        return Some(Parsed { nums, pre: None });
    }
    let (rank, tail) = if let Some(t) = rest.strip_prefix("snapshot") {
        (0, t)
    } else if let Some(t) = rest
        .strip_prefix("pre-release")
        .or_else(|| rest.strip_prefix("pre"))
    {
        (1, t)
    } else {
        (2, rest.strip_prefix("rc")?)
    };
    let n = tail
        .trim_start_matches(['-', ' ', '_'])
        .parse::<u32>()
        .unwrap_or(0);
    Some(Parsed {
        nums,
        pre: Some((rank, n)),
    })
}

/// Compares two version ids. Ids that cannot be parsed (e.g. `24w14a`,
/// `b1.7.3`) fall back to plain string comparison.
pub fn compare(a: &str, b: &str) -> Ordering {
    match (parse(a), parse(b)) {
        (Some(pa), Some(pb)) => {
            let len = pa.nums.len().max(pb.nums.len());
            for i in 0..len {
                let x = pa.nums.get(i).copied().unwrap_or(0);
                let y = pb.nums.get(i).copied().unwrap_or(0);
                match x.cmp(&y) {
                    Ordering::Equal => continue,
                    o => return o,
                }
            }
            match (pa.pre, pb.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(x), Some(y)) => x.cmp(&y),
            }
        }
        _ => a.cmp(b),
    }
}

/// `true` if `id >= min`.
pub fn at_least(id: &str, min: &str) -> bool {
    compare(id, min) != Ordering::Less
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_both_schemes() {
        let mut v = vec![
            "26.3",
            "1.21.11",
            "1.8.9",
            "26.1",
            "1.21",
            "26.3-rc-3",
            "26.4-snapshot-2",
            "26.3-pre-2",
            "1.12.2",
            "26.1.2",
        ];
        v.sort_by(|a, b| compare(a, b));
        assert_eq!(
            v,
            [
                "1.8.9",
                "1.12.2",
                "1.21",
                "1.21.11",
                "26.1",
                "26.1.2",
                "26.3-pre-2",
                "26.3-rc-3",
                "26.3",
                "26.4-snapshot-2"
            ]
        );
    }

    #[test]
    fn legacy_prerelease_names() {
        assert_eq!(compare("1.21-pre1", "1.21"), Ordering::Less);
        assert_eq!(compare("1.21-pre1", "1.21-rc1"), Ordering::Less);
        assert_eq!(compare("1.14 Pre-Release 2", "1.14"), Ordering::Less);
        assert_eq!(compare("1.21.0", "1.21"), Ordering::Equal);
    }

    #[test]
    fn at_least_helper() {
        assert!(at_least("26.3", "1.13"));
        assert!(at_least("1.13", "1.13"));
        assert!(!at_least("1.12.2", "1.13"));
    }
}
