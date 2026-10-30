//! Desktop endpoint login policy. Library/offline callers retain LocalGame::play's zero profile.
use crate::{Result, game::LoginProfile, game::server_endpoint};
use std::net::IpAddr;
use url::Host;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Auto,
    Official,
    Zero,
}
impl std::str::FromStr for Mode {
    type Err = String;
    fn from_str(value: &str) -> Result<Self> {
        match value {
            "auto" => Ok(Self::Auto),
            "official" => Ok(Self::Official),
            "zero" => Ok(Self::Zero),
            _ => Err("--login-profile requires auto, official or zero".into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Official,
    ZeroLoopback,
    ZeroForced,
    ZeroFallback,
}
impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Official => "official",
            Self::ZeroLoopback => "zero-loopback",
            Self::ZeroForced => "zero-forced",
            Self::ZeroFallback => "zero-fallback",
        }
    }
    pub fn fallback_message(self) -> Option<&'static str> {
        (self == Self::ZeroFallback)
            .then_some("Official login profile unavailable; using zero credentials.")
    }
}

pub struct Resolved {
    pub profile: LoginProfile,
    pub source: Source,
}

/// Auto avoids all site requests for literal loopback hosts; overrides are explicit.
pub fn decision(endpoint: &str, mode: Mode) -> Result<Source> {
    let url = server_endpoint(endpoint)?;
    let loopback = match url.host() {
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(ip)) => IpAddr::V4(ip).is_loopback(),
        Some(Host::Ipv6(ip)) => IpAddr::V6(ip).is_loopback(),
        None => false,
    };
    Ok(match mode {
        Mode::Zero => Source::ZeroForced,
        Mode::Auto if loopback => Source::ZeroLoopback,
        Mode::Auto | Mode::Official => Source::Official,
    })
}

/// Fetch errors are intentionally discarded: diagnostics must never expose profile data.
pub fn resolve_with(
    endpoint: &str,
    mode: Mode,
    fetch: impl FnOnce() -> Result<LoginProfile>,
) -> Result<Resolved> {
    let mut source = decision(endpoint, mode)?;
    let profile = if source == Source::Official {
        fetch().unwrap_or_else(|_| {
            source = Source::ZeroFallback;
            LoginProfile::default()
        })
    } else {
        LoginProfile::default()
    };
    Ok(Resolved { profile, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_and_overrides_do_not_restrict_endpoint_hosts() {
        for host in ["127.0.0.1", "127.99.8.7", "[::1]", "localhost", "LOCALHOST"] {
            let endpoint = format!("ws://{host}:19491/slither");
            assert_eq!(
                decision(&endpoint, Mode::Auto).unwrap(),
                Source::ZeroLoopback
            );
            assert_eq!(
                decision(&endpoint, Mode::Official).unwrap(),
                Source::Official
            );
            let resolved = resolve_with(&endpoint, Mode::Auto, || panic!("offline")).unwrap();
            assert_eq!(resolved.profile.client_version, 0);
            assert_eq!(resolved.profile.page_echo, [0; 20]);
        }
        for host in [
            "192.0.2.1",
            "10.0.0.1",
            "[2001:db8::1]",
            "game.example",
            "slither.io",
        ] {
            let endpoint = format!("ws://{host}:19491/slither");
            assert_eq!(decision(&endpoint, Mode::Auto).unwrap(), Source::Official);
            assert_eq!(
                decision(&endpoint, Mode::Official).unwrap(),
                Source::Official
            );
        }
        for host in ["localhost", "192.0.2.1", "[::1]"] {
            let resolved = resolve_with(&format!("ws://{host}:19491/slither"), Mode::Zero, || {
                panic!("forced zero must not fetch")
            })
            .unwrap();
            assert_eq!(resolved.source, Source::ZeroForced);
            assert_eq!(resolved.profile.page_echo, [0; 20]);
        }
    }

    #[test]
    fn success_and_failure_select_the_profile_and_one_safe_message() {
        let endpoint = "ws://game.example:19491/slither";
        let official = resolve_with(endpoint, Mode::Auto, || {
            Ok(LoginProfile {
                client_version: 4242,
                page_echo: [123; 20],
            })
        })
        .unwrap();
        assert_eq!(official.source, Source::Official);
        assert_eq!(official.profile.client_version, 4242);
        assert_eq!(official.profile.page_echo, [123; 20]);
        assert!(official.source.fallback_message().is_none());
        let fallback =
            resolve_with(endpoint, Mode::Auto, || Err("private failure".into())).unwrap();
        assert_eq!(fallback.source, Source::ZeroFallback);
        assert_eq!(fallback.profile.client_version, 0);
        assert_eq!(fallback.profile.page_echo, [0; 20]);
        let messages: Vec<_> = fallback.source.fallback_message().into_iter().collect();
        assert_eq!(
            messages,
            ["Official login profile unavailable; using zero credentials."]
        );
    }
}
