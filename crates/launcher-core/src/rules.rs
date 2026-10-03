//! Evaluation of Mojang `rules` arrays (libraries and arguments).
//!
//! Semantics (as implemented by the official launcher):
//! - no rules → allowed;
//! - otherwise start disallowed and walk the rules in order; every rule whose
//!   conditions all match sets the result to its `action`.

use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::os::OsInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OsRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<OsRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<HashMap<String, bool>>,
}

/// Everything a rule can test against.
#[derive(Debug, Clone)]
pub struct RuleEnv {
    pub os: OsInfo,
    /// Enabled launcher features (`has_custom_resolution`, `is_demo_user`, …).
    /// Missing features count as `false`.
    pub features: HashMap<String, bool>,
}

impl RuleEnv {
    pub fn new(os: OsInfo) -> Self {
        Self {
            os,
            features: HashMap::new(),
        }
    }

    pub fn with_feature(mut self, name: &str, on: bool) -> Self {
        self.features.insert(name.to_owned(), on);
        self
    }
}

pub fn allowed(rules: &[Rule], env: &RuleEnv) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut result = false;
    for rule in rules {
        if rule_matches(rule, env) {
            result = rule.action == RuleAction::Allow;
        }
    }
    result
}

fn rule_matches(rule: &Rule, env: &RuleEnv) -> bool {
    if let Some(os) = &rule.os {
        if let Some(name) = &os.name
            && !os_name_matches(name, env.os.name)
        {
            return false;
        }
        if let Some(arch) = &os.arch
            && !arch_matches(arch, env.os.arch)
        {
            return false;
        }
        if let Some(pattern) = &os.version {
            // An invalid regex never matches (the official launcher ignores it too).
            let ok = Regex::new(pattern)
                .map(|re| re.is_match(&env.os.version))
                .unwrap_or(false);
            if !ok {
                return false;
            }
        }
    }
    if let Some(features) = &rule.features {
        for (name, wanted) in features {
            let have = env.features.get(name).copied().unwrap_or(false);
            if have != *wanted {
                return false;
            }
        }
    }
    true
}

fn os_name_matches(rule: &str, host: &str) -> bool {
    match rule {
        "osx" | "macos" => host == "osx",
        other => other == host,
    }
}

fn arch_matches(rule: &str, host: &str) -> bool {
    match rule {
        "x86" => host == "x86",
        "x86_64" | "amd64" => host == "x86_64",
        "arm64" | "aarch64" => host == "arm64",
        other => other == host,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(json: &str) -> Vec<Rule> {
        serde_json::from_str(json).unwrap()
    }

    fn win() -> RuleEnv {
        RuleEnv::new(OsInfo::fake("windows", "x86_64"))
    }

    #[test]
    fn empty_rules_allow() {
        assert!(allowed(&[], &win()));
    }

    #[test]
    fn os_allow_list() {
        let r = rules(r#"[{"action":"allow","os":{"name":"osx"}}]"#);
        assert!(!allowed(&r, &win()));
        assert!(allowed(&r, &RuleEnv::new(OsInfo::fake("osx", "arm64"))));
    }

    #[test]
    fn allow_then_disallow_osx() {
        // lwjgl 2.9.4 in 1.12.2
        let r = rules(r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]"#);
        assert!(allowed(&r, &win()));
        assert!(!allowed(&r, &RuleEnv::new(OsInfo::fake("osx", "x86_64"))));
    }

    #[test]
    fn arch_rule() {
        let r = rules(r#"[{"action":"allow","os":{"arch":"x86"}}]"#);
        assert!(!allowed(&r, &win()));
        assert!(allowed(&r, &RuleEnv::new(OsInfo::fake("windows", "x86"))));
    }

    #[test]
    fn version_regex() {
        let r = rules(r#"[{"action":"allow","os":{"name":"osx","version":"^10\\.5\\.\\d$"}}]"#);
        let mut env = RuleEnv::new(OsInfo::fake("osx", "x86_64"));
        env.os.version = "10.5.8".into();
        assert!(allowed(&r, &env));
        env.os.version = "14.1".into();
        assert!(!allowed(&r, &env));
    }

    #[test]
    fn features() {
        let r = rules(r#"[{"action":"allow","features":{"has_custom_resolution":true}}]"#);
        assert!(!allowed(&r, &win()));
        assert!(allowed(
            &r,
            &win().with_feature("has_custom_resolution", true)
        ));
        let demo = rules(r#"[{"action":"allow","features":{"is_demo_user":true}}]"#);
        assert!(!allowed(
            &demo,
            &win().with_feature("has_custom_resolution", true)
        ));
    }
}
