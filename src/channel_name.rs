//! Parsing of channel names.
//!
//! A channel name is `<base>[+<variant>...][~<arch>]`.
//!
//! The base names a versions-database channel, `nightly`, `x.y-nightly`,
//! or `pr<number>`. Variants select a build configuration; their exact spelling is
//! the sorted token order shown by listing. The optional architecture suffix selects
//! a build for another architecture. Availability is resolved separately.

use anyhow::{bail, Result};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChannelBase {
    /// `nightly` (`series: None`) or `x.y-nightly` (`series: Some((x, y))`).
    Nightly { series: Option<(u64, u64)> },
    /// `pr<number>`.
    Pr(u64),
    /// Any other channel; the versions database decides whether it exists.
    Db(String),
}

impl ChannelBase {
    fn parse(base: &str) -> Self {
        if base == "nightly" {
            return ChannelBase::Nightly { series: None };
        }
        if let Some(series) = base.strip_suffix("-nightly") {
            if let Some((major, minor)) = series.split_once('.') {
                if let (Ok(major), Ok(minor)) = (major.parse(), minor.parse()) {
                    return ChannelBase::Nightly {
                        series: Some((major, minor)),
                    };
                }
            }
        }
        if let Some(number) = base.strip_prefix("pr") {
            if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) {
                if let Ok(number) = number.parse() {
                    return ChannelBase::Pr(number);
                }
            }
        }
        ChannelBase::Db(base.to_string())
    }
}

impl fmt::Display for ChannelBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChannelBase::Nightly { series: None } => f.write_str("nightly"),
            ChannelBase::Nightly {
                series: Some((major, minor)),
            } => write!(f, "{}.{}-nightly", major, minor),
            ChannelBase::Pr(number) => write!(f, "pr{}", number),
            ChannelBase::Db(name) => f.write_str(name),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelName {
    pub base: ChannelBase,
    /// The `+<variant>` parts, preserved exactly as supplied.
    pub variants: Vec<String>,
    /// The `~<arch>` suffix, if any. Kept verbatim: for `Db` channels it is
    /// part of the versions database key, and for the other channels the
    /// build server decides which architectures exist.
    pub arch: Option<String>,
}

impl ChannelName {
    pub fn parse(channel: &str) -> Result<Self> {
        let (name, arch) = match channel.split_once('~') {
            Some((name, arch)) => (name, Some(arch)),
            None => (channel, None),
        };
        let mut parts = name.split('+');
        let base = parts.next().unwrap_or_default();
        let variants: Vec<String> = parts.map(str::to_string).collect();

        let valid_variant = |variant: &String| {
            !variant.is_empty() && variant.bytes().all(|b| b.is_ascii_alphanumeric())
        };
        if arch.is_some_and(|arch| arch.contains('+')) {
            bail!(
                "'{}' is not a valid channel name: build variants go before the `~arch` suffix, e.g. `{}+opt~x64`.",
                channel,
                base
            );
        }
        if base.is_empty()
            || !variants.iter().all(valid_variant)
            || arch.is_some_and(|arch| arch.is_empty() || arch.contains('~'))
        {
            bail!("'{}' is not a valid channel name.", channel);
        }

        Ok(ChannelName {
            base: ChannelBase::parse(base),
            variants,
            arch: arch.map(str::to_string),
        })
    }

    pub fn is_nightly(&self) -> bool {
        matches!(self.base, ChannelBase::Nightly { .. })
    }

    pub fn is_pr(&self) -> bool {
        matches!(self.base, ChannelBase::Pr(_))
    }

    /// Nightly and PR channels are downloaded directly from the build server
    /// rather than resolved through the versions database.
    pub fn is_direct_download(&self) -> bool {
        self.is_nightly() || self.is_pr()
    }

    /// The `+<variant>...` part of the name, empty for the standard build.
    pub fn variant_suffix(&self) -> String {
        self.variants.iter().map(|v| format!("+{}", v)).collect()
    }
}

impl fmt::Display for ChannelName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.base, self.variant_suffix())?;
        if let Some(arch) = &self.arch {
            write!(f, "~{}", arch)?;
        }
        Ok(())
    }
}

pub fn is_nightly_channel(channel: &str) -> bool {
    ChannelName::parse(channel).is_ok_and(|name| name.is_nightly())
}

pub fn is_pr_channel(channel: &str) -> bool {
    ChannelName::parse(channel).is_ok_and(|name| name.is_pr())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(channel: &str) -> ChannelName {
        ChannelName::parse(channel).unwrap()
    }

    #[test]
    fn parses_nightly_channels() {
        assert_eq!(parse("nightly").base, ChannelBase::Nightly { series: None });
        assert_eq!(
            parse("1.13-nightly").base,
            ChannelBase::Nightly {
                series: Some((1, 13))
            }
        );
        assert_eq!(parse("nightly~x86").arch.as_deref(), Some("x86"));
        assert!(parse("1.13-nightly~aarch64").is_nightly());
    }

    #[test]
    fn parses_pr_channels() {
        assert_eq!(parse("pr12345").base, ChannelBase::Pr(12345));
        assert!(parse("pr1~x64").is_pr());
        assert_eq!(parse("pr").base, ChannelBase::Db("pr".to_string()));
        assert_eq!(parse("pr12a").base, ChannelBase::Db("pr12a".to_string()));
    }

    #[test]
    fn everything_else_is_a_db_channel() {
        for channel in [
            "release",
            "lts",
            "1",
            "1.13",
            "1.13.2",
            "1.13.0-rc1",
            "latest",
        ] {
            assert_eq!(parse(channel).base, ChannelBase::Db(channel.to_string()));
            assert!(!parse(channel).is_direct_download());
        }
        assert_eq!(parse("1.13~x64").base, ChannelBase::Db("1.13".to_string()));
        assert_eq!(parse("1.13~x64").arch.as_deref(), Some("x64"));
        // Not quite a nightly channel, so the versions database gets to decide.
        assert_eq!(
            parse("1.x-nightly").base,
            ChannelBase::Db("1.x-nightly".to_string())
        );
    }

    #[test]
    fn parses_variants() {
        let name = parse("1.13-nightly+opt");
        assert!(name.is_nightly());
        assert_eq!(name.variants, ["opt"]);
        assert_eq!(name.arch, None);

        let name = parse("nightly+opt+assert~x64");
        assert_eq!(name.variants, ["opt", "assert"]);
        assert_eq!(name.arch.as_deref(), Some("x64"));
        assert_ne!(name, parse("nightly+assert+opt+opt~x64"));
        assert_eq!(name.variant_suffix(), "+opt+assert");

        // Variants parse on any base; whether they exist is decided later.
        assert_eq!(
            parse("release+opt").base,
            ChannelBase::Db("release".to_string())
        );
        assert_eq!(parse("pr123+opt").base, ChannelBase::Pr(123));
        assert!(parse("release").variants.is_empty());
    }

    #[test]
    fn rejects_malformed_names() {
        for channel in [
            "",
            "~x64",
            "nightly~",
            "nightly~x64~x86",
            "nightly+",
            "+opt",
            "nightly++opt",
            "nightly+opt-2",
            "nightly~x64+opt",
        ] {
            assert!(ChannelName::parse(channel).is_err(), "{channel:?}");
        }
    }

    #[test]
    fn display_round_trips() {
        for channel in [
            "nightly",
            "1.13-nightly~x86",
            "pr12345",
            "pr7~aarch64",
            "release",
            "1.13.2~x64",
            "nightly+opt",
            "1.13-nightly+assert+opt~x64",
        ] {
            assert_eq!(parse(channel).to_string(), channel);
        }
    }

    #[test]
    fn predicates() {
        assert!(is_nightly_channel("nightly~x64"));
        assert!(is_nightly_channel("1.13-nightly"));
        assert!(is_nightly_channel("nightly+opt"));
        assert!(!is_nightly_channel("nightly~"));
        assert!(!is_nightly_channel("release"));
        assert!(is_pr_channel("pr123"));
        assert!(!is_pr_channel("pr"));
    }
}
