//! Maven coordinates: `group:artifact:version[:classifier][@ext]`.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Coordinate {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub extension: String,
}

impl Coordinate {
    pub fn parse(s: &str) -> Option<Self> {
        let (main, extension) = match s.split_once('@') {
            Some((m, e)) => (m, e.to_owned()),
            None => (s, "jar".to_owned()),
        };
        let mut parts = main.split(':');
        let group = parts.next()?.to_owned();
        let artifact = parts.next()?.to_owned();
        let version = parts.next()?.to_owned();
        let classifier = parts.next().map(str::to_owned);
        if parts.next().is_some() || group.is_empty() || artifact.is_empty() || version.is_empty() {
            return None;
        }
        Some(Self {
            group,
            artifact,
            version,
            classifier,
            extension,
        })
    }

    pub fn with_classifier(&self, classifier: &str) -> Self {
        Self {
            classifier: Some(classifier.to_owned()),
            ..self.clone()
        }
    }

    /// Repository-relative path with `/` separators.
    pub fn path(&self) -> String {
        let file = match &self.classifier {
            Some(c) => format!(
                "{}-{}-{}.{}",
                self.artifact, self.version, c, self.extension
            ),
            None => format!("{}-{}.{}", self.artifact, self.version, self.extension),
        };
        format!(
            "{}/{}/{}/{}",
            self.group.replace('.', "/"),
            self.artifact,
            self.version,
            file
        )
    }

    /// Identity used for classpath de-duplication: the same library in two
    /// versions must collapse to one entry (the first one, i.e. the loader's).
    pub fn dedup_key(&self) -> String {
        match &self.classifier {
            Some(c) => format!("{}:{}:{}", self.group, self.artifact, c),
            None => format!("{}:{}", self.group, self.artifact),
        }
    }
}

impl fmt::Display for Coordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.group, self.artifact, self.version)?;
        if let Some(c) = &self.classifier {
            write!(f, ":{c}")?;
        }
        if self.extension != "jar" {
            write!(f, "@{}", self.extension)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_path() {
        let c = Coordinate::parse("org.lwjgl:lwjgl:3.4.3:natives-windows").unwrap();
        assert_eq!(
            c.path(),
            "org/lwjgl/lwjgl/3.4.3/lwjgl-3.4.3-natives-windows.jar"
        );
        assert_eq!(c.dedup_key(), "org.lwjgl:lwjgl:natives-windows");

        let z = Coordinate::parse("de.oceanlabs.mcp:mcp_config:1.20.1@zip").unwrap();
        assert_eq!(
            z.path(),
            "de/oceanlabs/mcp/mcp_config/1.20.1/mcp_config-1.20.1.zip"
        );
        assert_eq!(z.to_string(), "de.oceanlabs.mcp:mcp_config:1.20.1@zip");
    }

    #[test]
    fn rejects_garbage() {
        assert!(Coordinate::parse("nope").is_none());
        assert!(Coordinate::parse("a:b").is_none());
        assert!(Coordinate::parse("a:b:c:d:e").is_none());
    }
}
