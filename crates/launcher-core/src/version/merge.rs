//! `inheritsFrom` merging: a child profile (loader) layered over its parent.
//!
//! - scalar fields: child wins when present
//! - `arguments.game/jvm`: parent first, then child
//! - `libraries`: child first, then parent; classpath de-duplication (first
//!   occurrence wins) later lets loader libraries override vanilla ones
//! - `minecraftArguments`: child wins (loaders repeat the full string)

use super::profile::{Arguments, VersionJson};

pub fn merge(child: VersionJson, parent: VersionJson) -> VersionJson {
    let arguments = match (parent.arguments, child.arguments) {
        (None, None) => None,
        (Some(a), None) | (None, Some(a)) => Some(a),
        (Some(p), Some(c)) => Some(Arguments {
            game: p.game.into_iter().chain(c.game).collect(),
            jvm: p.jvm.into_iter().chain(c.jvm).collect(),
        }),
    };
    VersionJson {
        id: child.id,
        inherits_from: None,
        kind: child.kind.or(parent.kind),
        main_class: child.main_class.or(parent.main_class),
        minecraft_arguments: child.minecraft_arguments.or(parent.minecraft_arguments),
        arguments,
        libraries: child
            .libraries
            .into_iter()
            .chain(parent.libraries)
            .collect(),
        asset_index: child.asset_index.or(parent.asset_index),
        assets: child.assets.or(parent.assets),
        downloads: child.downloads.or(parent.downloads),
        java_version: child.java_version.or(parent.java_version),
        logging: child.logging.or(parent.logging),
        jar: child.jar.or(parent.jar),
        release_time: child.release_time.or(parent.release_time),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_overrides_and_arguments_concatenate() {
        let parent: VersionJson = serde_json::from_str(
            r#"{"id":"26.3","type":"release","mainClass":"net.minecraft.client.main.Main",
                "arguments":{"game":["--username","${auth_player_name}"],"jvm":["-cp","${classpath}"]},
                "libraries":[{"name":"org.ow2.asm:asm:9.6"},{"name":"com.google.guava:guava:33"}],
                "javaVersion":{"majorVersion":25},"assets":"34"}"#,
        )
        .unwrap();
        let child: VersionJson = serde_json::from_str(
            r#"{"id":"fabric-loader-0.19.5-26.3","inheritsFrom":"26.3",
                "mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient",
                "arguments":{"game":[],"jvm":["-DFabricMcEmu= net.minecraft.client.main.Main "]},
                "libraries":[{"name":"org.ow2.asm:asm:9.10.1","url":"https://maven.fabricmc.net/"}]}"#,
        )
        .unwrap();

        let m = merge(child, parent);
        assert_eq!(m.id, "fabric-loader-0.19.5-26.3");
        assert!(m.inherits_from.is_none());
        assert_eq!(
            m.main_class.as_deref(),
            Some("net.fabricmc.loader.impl.launch.knot.KnotClient")
        );
        assert_eq!(m.kind.as_deref(), Some("release"));
        assert_eq!(m.java_major(), 25);
        assert_eq!(m.arguments.as_ref().unwrap().jvm.len(), 3);
        // Loader's asm first so it wins classpath de-duplication.
        assert_eq!(m.libraries[0].name, "org.ow2.asm:asm:9.10.1");
        assert_eq!(m.libraries.len(), 3);
    }
}
