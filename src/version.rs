/* Pi-hole: A black hole for Internet advertisements
*  (c) 2018 Pi-hole, LLC (https://pi-hole.net)
*  Network-wide ad blocking via your own hardware.
*
*  API
*  Version endpoint
*
*  This file is copyright under the latest version of the EUPL.
*  Please see LICENSE file for your rights under this license. */

use rocket::State;
use config::Config;
use config::PiholeFile;
use ftl::FtlConnectionType;
use util;
use std::io::Read;
use web::WebAssets;

/// Get the versions of all Pi-hole systems
#[get("/version")]
pub fn version(config: State<Config>, ftl: State<FtlConnectionType>) -> util::Reply {
    // Core
    // Web
    // FTL
    // API
    let core_version = read_core_version(&config).unwrap_or_default();
    let web_version = read_web_version().unwrap_or_default();

    util::reply_data(json!({
        "core": core_version,
        "web": web_version
    }))
}

/// Read Web version information from the `VERSION` file in the web assets.
fn read_web_version() -> Option<Version> {
   WebAssets::get("VERSION")
        .and_then(|raw| String::from_utf8(raw).ok())
        .and_then(|version| parse_web_version(&version))
}

/// Parse Web version information from the string.
/// The string should be in the format "TAG BRANCH COMMIT".
fn parse_web_version(version_str: &str) -> Option<Version> {
    // Trim to remove possible newline
    let version_split: Vec<&str> = version_str
        .trim_right_matches("\n")
        .split(" ")
        .collect();

    if version_split.len() != 3 {
        return None;
    }

    Some(Version {
        tag: version_split[0].to_owned(),
        branch: version_split[1].to_owned(),
        hash: version_split[2].to_owned()
    })
}

/// Read Core version information from the file system
fn read_core_version(config: &Config) -> Option<Version> {
    // Read the version files
    let mut local_versions = String::new();
    let mut local_branches = String::new();
    config.read_file(PiholeFile::LocalVersions)
        .ok()
        .and_then(|mut f| f.read_to_string(&mut local_versions).ok());
    config.read_file(PiholeFile::LocalBranches)
        .ok()
        .and_then(|mut f| f.read_to_string(&mut local_branches).ok());

    // These files are structured as "CORE WEB FTL", but we only want Core's data
    let git_version = local_versions.split(" ").next().unwrap_or_default();
    let core_branch = local_branches.split(" ").next().unwrap_or_default();

    // Parse the version data
    parse_git_version(git_version, core_branch)
}

/// Parse version data from the output of `git describe` (stored in `PiholeFile::LocalVersions`).
/// The string is in the form "TAG-NUMBER-COMMIT".
fn parse_git_version(git_version: &str, branch: &str) -> Option<Version> {
    let split: Vec<&str> = git_version.split("-").collect();

    if split.len() != 3 {
        return None;
    }

    // Only set the tag if this is the tagged commit (we are 0 commits after the tag)
    let tag = if split[1] == "0" { split[0] } else { "" };

    Some(Version {
        tag: tag.to_owned(),
        branch: branch.to_owned(),
        // Ignore the beginning "g" character
        hash: split[2].get(1..).unwrap_or_default().to_owned()
    })
}

#[derive(Debug, PartialEq, Serialize, Default)]
struct Version {
    tag: String,
    branch: String,
    hash: String
}

#[cfg(test)]
mod tests {
    use super::{Version, parse_git_version, parse_web_version};
    use testing::TestConfigBuilder;
    use config::PiholeFile;
    use config::Config;
    use version::read_core_version;

    #[test]
    fn test_parse_web_version_dev() {
        assert_eq!(
            parse_web_version(" development d2037fd"),
            Some(Version {
                tag: "".to_owned(),
                branch: "development".to_owned(),
                hash: "d2037fd".to_owned()
            })
        )
    }

    #[test]
    fn test_parse_web_version_release() {
        assert_eq!(
            parse_web_version("v1.0.0 master abcdefg"),
            Some(Version {
                tag: "v1.0.0".to_owned(),
                branch: "master".to_owned(),
                hash: "abcdefg".to_owned()
            })
        )
    }

    #[test]
    fn test_parse_web_version_invalid() {
        assert_eq!(parse_web_version("invalid data"), None)
    }

    #[test]
    fn test_parse_web_version_newline() {
        assert_eq!(
            parse_web_version(" development d2037fd\n"),
            Some(Version {
                tag: "".to_owned(),
                branch: "development".to_owned(),
                hash: "d2037fd".to_owned()
            })
        )
    }

    #[test]
    fn test_read_core_version_valid() {
        let test_config = Config::Test(
            TestConfigBuilder::new()
                .file(
                    PiholeFile::LocalVersions,
                    "v3.3.1-219-g6689e00 v3.3-190-gf7e1a28 vDev-d06deca"
                )
                .file(
                    PiholeFile::LocalBranches,
                    "development devel tweak/getClientNames"
                )
                .build()
        );

        assert_eq!(
            read_core_version(&test_config),
            Some(Version {
                tag: "".to_owned(),
                branch: "development".to_owned(),
                hash: "6689e00".to_owned()
            })
        )
    }

    #[test]
    fn test_read_core_version_invalid() {
        let test_config = Config::Test(
            TestConfigBuilder::new()
                .file(
                    PiholeFile::LocalVersions,
                    "invalid v3.3-190-gf7e1a28 vDev-d06deca"
                )
                .file(
                    PiholeFile::LocalBranches,
                    "development devel tweak/getClientNames"
                )
                .build()
        );

        assert_eq!(read_core_version(&test_config), None)
    }

    #[test]
    fn test_parse_git_version_release() {
        assert_eq!(
            parse_git_version("v3.3.1-0-gfbee18e", "master"),
            Some(Version {
                tag: "v3.3.1".to_owned(),
                branch: "master".to_owned(),
                hash: "fbee18e".to_owned()
            })
        )
    }

    #[test]
    fn test_parse_git_version_dev() {
        assert_eq!(
            parse_git_version("v3.3.1-222-gd9c924b", "development"),
            Some(Version {
                tag: "".to_owned(),
                branch: "development".to_owned(),
                hash: "d9c924b".to_owned()
            })
        )
    }

    #[test]
    fn test_parse_git_version_invalid() {
        assert_eq!(parse_git_version("invalid data", "branch"), None)
    }
}
