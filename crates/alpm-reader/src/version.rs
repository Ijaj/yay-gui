//! Port of libalpm's `alpm_pkg_vercmp` / `rpmvercmp`.

use std::cmp::Ordering;

/// Compare two `[epoch:]pkgver[-pkgrel]` strings exactly like `vercmp(8)`.
///
/// A missing epoch is `0`; the release is compared only if both sides have one.
/// Note: unlike rpm, `~` is just a separator here (as in libalpm).
pub fn vercmp(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    let (ea, va, ra) = parse_evr(a);
    let (eb, vb, rb) = parse_evr(b);
    rpmvercmp(ea, eb)
        .then_with(|| rpmvercmp(va, vb))
        .then_with(|| match (ra, rb) {
            (Some(x), Some(y)) => rpmvercmp(x, y),
            _ => Ordering::Equal,
        })
}

fn parse_evr(s: &str) -> (&str, &str, Option<&str>) {
    let digits = s.bytes().take_while(u8::is_ascii_digit).count();
    let (epoch, rest) = if s[digits..].starts_with(':') {
        (&s[..digits], &s[digits + 1..])
    } else {
        ("0", s)
    };
    let epoch = if epoch.is_empty() { "0" } else { epoch };
    match rest.rfind('-') {
        Some(i) => (epoch, &rest[..i], Some(&rest[i + 1..])),
        None => (epoch, rest, None),
    }
}

fn rpmvercmp(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (mut one, mut two) = (0usize, 0usize);
    let (mut p1, mut p2) = (0usize, 0usize);
    let at = |s: &[u8], i: usize| s.get(i).copied();
    while one < a.len() && two < b.len() {
        while one < a.len() && !a[one].is_ascii_alphanumeric() {
            one += 1;
        }
        while two < b.len() && !b[two].is_ascii_alphanumeric() {
            two += 1;
        }
        if one >= a.len() || two >= b.len() {
            break;
        }
        // Different separator lengths: the shorter one is older.
        if one - p1 != two - p2 {
            return (one - p1).cmp(&(two - p2));
        }
        p1 = one;
        p2 = two;
        let isnum = a[p1].is_ascii_digit();
        let class: fn(&u8) -> bool = if isnum {
            u8::is_ascii_digit
        } else {
            u8::is_ascii_alphabetic
        };
        while at(a, p1).as_ref().is_some_and(class) {
            p1 += 1;
        }
        while at(b, p2).as_ref().is_some_and(class) {
            p2 += 1;
        }
        // Segment types differ: numeric is newer than alpha.
        if two == p2 {
            return if isnum {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        let (mut s1, mut s2) = (&a[one..p1], &b[two..p2]);
        if isnum {
            while s1.first() == Some(&b'0') {
                s1 = &s1[1..];
            }
            while s2.first() == Some(&b'0') {
                s2 = &s2[1..];
            }
            if s1.len() != s2.len() {
                return s1.len().cmp(&s2.len());
            }
        }
        match s1.cmp(s2) {
            Ordering::Equal => {}
            o => return o,
        }
        one = p1;
        two = p2;
    }
    if one >= a.len() && two >= b.len() {
        return Ordering::Equal;
    }
    // Leftover alpha never beats nothing; leftover non-alpha does.
    let c1 = a.get(one).copied();
    let c2 = b.get(two).copied();
    if (c1.is_none() && !c2.is_some_and(|c| c.is_ascii_alphabetic()))
        || c1.is_some_and(|c| c.is_ascii_alphabetic())
    {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // (a, b, expected sign) — from pacman's test/util/vercmptest.sh.
    const CASES: &[(&str, &str, i8)] = &[
        // all similar length, no pkgrel
        ("1.5.0", "1.5.0", 0),
        ("1.5.1", "1.5.0", 1),
        // mixed length
        ("1.5.1", "1.5", 1),
        // with pkgrel, simple
        ("1.5.0-1", "1.5.0-1", 0),
        ("1.5.0-1", "1.5.0-2", -1),
        ("1.5.0-1", "1.5.1-1", -1),
        ("1.5.0-2", "1.5.1-1", -1),
        // with pkgrel, mixed lengths
        ("1.5-1", "1.5.1-1", -1),
        ("1.5-2", "1.5.1-1", -1),
        ("1.5-2", "1.5.1-2", -1),
        // mixed pkgrel inclusion
        ("1.5", "1.5-1", 0),
        ("1.5-1", "1.5", 0),
        ("1.1-1", "1.1", 0),
        ("1.0-1", "1.1", -1),
        ("1.1-1", "1.0", 1),
        // alphanumeric versions
        ("1.5b-1", "1.5-1", -1),
        ("1.5b", "1.5", -1),
        ("1.5b-1", "1.5", -1),
        ("1.5b", "1.5.1", -1),
        // from the manpage
        ("1.0a", "1.0alpha", -1),
        ("1.0alpha", "1.0b", -1),
        ("1.0b", "1.0beta", -1),
        ("1.0beta", "1.0rc", -1),
        ("1.0rc", "1.0", -1),
        // going crazy? alpha-dotted versions
        ("1.5.a", "1.5", 1),
        ("1.5.b", "1.5.a", 1),
        ("1.5.1", "1.5.b", 1),
        // alpha dots and dashes
        ("1.5.b-1", "1.5.b", 0),
        ("1.5-1", "1.5.b", -1),
        // same/similar content, differing separators
        ("2.0", "2_0", 0),
        ("2.0_a", "2_0.a", 0),
        ("2.0a", "2.0.a", -1),
        ("2___a", "2_a", 1),
        // epochs
        ("0:1.0", "0:1.0", 0),
        ("0:1.0", "0:1.1", -1),
        ("1:1.0", "0:1.0", 1),
        ("1:1.0", "0:1.1", 1),
        ("1:1.0", "2:1.1", -1),
        // epoch + sometimes present pkgrel
        ("1:1.0", "0:1.0-1", 1),
        ("1:1.0-1", "0:1.1-1", 1),
        // epoch included on one version but not the other
        ("0:1.0", "1.0", 0),
        ("0:1.0", "1.1", -1),
        ("0:1.1", "1.0", 1),
        ("1:1.0", "1.0", 1),
        ("1:1.0", "1.1", 1),
        ("1:1.1", "1.1", 1),
        // numeric details
        ("1.0", "1.00", 0),
        ("1.010", "1.01", 1),
        ("1.9", "1.10", -1),
        ("1.0", "1.0.0", -1),
        ("1.0", "1..0", -1),
        // tilde is a plain separator in libalpm
        ("1.0~rc1", "1.0", 1),
        ("1.0~rc1", "1.0.rc1", 0),
        ("", "", 0),
    ];

    #[test]
    fn table() {
        for &(a, b, want) in CASES {
            let got = vercmp(a, b) as i8;
            assert_eq!(got, want, "vercmp({a:?}, {b:?})");
            if a != b {
                // antisymmetry
                assert_eq!(vercmp(b, a) as i8, -want, "vercmp({b:?}, {a:?})");
            }
        }
    }

    #[test]
    fn evr_parsing() {
        assert_eq!(parse_evr("1:2.0-3"), ("1", "2.0", Some("3")));
        assert_eq!(parse_evr("2.0"), ("0", "2.0", None));
        assert_eq!(parse_evr("a:2.0"), ("0", "a:2.0", None));
    }
}
