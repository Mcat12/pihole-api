// Pi-hole: A black hole for Internet advertisements
// (c) 2018 Pi-hole, LLC (https://pi-hole.net)
// Network-wide ad blocking via your own hardware.
//
// API
// Config File Structure
//
// This file is copyright under the latest version of the EUPL.
// Please see LICENSE file for your rights under this license.

use env::PiholeFile;
use failure::Fail;
use failure::ResultExt;
use rocket::config::LoggingLevel;
use std::fs::File;
use std::io::{self, prelude::*};
use std::net::Ipv4Addr;
use std::path::Path;
use std::str::FromStr;
use toml;
use util::{Error, ErrorKind};

/// The API config options
#[derive(Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    general: General,
    #[serde(default)]
    file_locations: Files
}

impl Config {
    /// Parse the config from the file located at `config_location`
    pub fn parse(config_location: &str) -> Result<Config, Error> {
        let mut buffer = String::new();

        // Read the file to a string, but return the default config if the file doesn't
        // exist
        let mut file = match File::open(config_location) {
            Ok(f) => f,
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => return Ok(Self::default()),
                _ => {
                    return Err(e.context(ErrorKind::FileRead(config_location.to_owned())))
                        .map_err(Error::from)
                }
            }
        };

        file.read_to_string(&mut buffer)
            .map_err(|e| e.context(ErrorKind::FileRead(config_location.to_owned())))?;

        let config = toml::from_str::<Config>(&buffer).context(ErrorKind::ConfigParsingError)?;

        if config.is_valid() {
            Ok(config)
        } else {
            Err(Error::from(ErrorKind::ConfigParsingError))
        }
    }

    /// Check if the config settings are valid
    pub fn is_valid(&self) -> bool {
        self.general.is_valid() && self.file_locations.is_valid()
    }

    /// Get the configured location of a file
    pub fn file_location(&self, file: PiholeFile) -> &str {
        match file {
            PiholeFile::DnsmasqConfig => &self.file_locations.dnsmasq_config,
            PiholeFile::Whitelist => &self.file_locations.whitelist,
            PiholeFile::Blacklist => &self.file_locations.blacklist,
            PiholeFile::Regexlist => &self.file_locations.regexlist,
            PiholeFile::SetupVars => &self.file_locations.setup_vars,
            PiholeFile::FtlConfig => &self.file_locations.ftl_config,
            PiholeFile::LocalVersions => &self.file_locations.local_versions,
            PiholeFile::LocalBranches => &self.file_locations.local_branches
        }
    }

    pub fn address(&self) -> &str {
        &self.general.address
    }

    pub fn port(&self) -> usize {
        self.general.port
    }

    pub fn log_level(&self) -> LoggingLevel {
        match self.general.log_level.as_str() {
            "critical" => LoggingLevel::Critical,
            "normal" => LoggingLevel::Normal,
            "debug" => LoggingLevel::Debug,
            // Should never happen, since config is validated on startup
            _ => LoggingLevel::Critical
        }
    }
}

/// Defines the deserialization of the "file_locations" section of the config
/// file. The default functions are generated by `default!`.
#[derive(Deserialize)]
pub struct Files {
    #[serde(default = "default_dnsmasq_config")]
    dnsmasq_config: String,
    #[serde(default = "default_whitelist")]
    whitelist: String,
    #[serde(default = "default_blacklist")]
    blacklist: String,
    #[serde(default = "default_regexlist")]
    regexlist: String,
    #[serde(default = "default_setup_vars")]
    setup_vars: String,
    #[serde(default = "default_ftl_config")]
    ftl_config: String,
    #[serde(default = "default_local_versions")]
    local_versions: String,
    #[serde(default = "default_local_branches")]
    local_branches: String
}

impl Default for Files {
    fn default() -> Self {
        Files {
            dnsmasq_config: default_dnsmasq_config(),
            whitelist: default_whitelist(),
            blacklist: default_blacklist(),
            regexlist: default_regexlist(),
            setup_vars: default_setup_vars(),
            ftl_config: default_ftl_config(),
            local_versions: default_local_versions(),
            local_branches: default_local_branches()
        }
    }
}

impl Files {
    fn is_valid(&self) -> bool {
        [
            &self.dnsmasq_config,
            &self.whitelist,
            &self.blacklist,
            &self.regexlist,
            &self.setup_vars,
            &self.ftl_config,
            &self.local_versions,
            &self.local_branches
        ].into_iter()
            .all(|file| Path::new(file).is_absolute())
    }
}

/// Create an `fn() -> String` default function for deserialization
macro_rules! default {
    ($fn_name:ident, $variant:ident) => {
        fn $fn_name() -> String {
            PiholeFile::$variant.default_location().to_owned()
        }
    };
}

default!(default_dnsmasq_config, DnsmasqConfig);
default!(default_whitelist, Whitelist);
default!(default_blacklist, Blacklist);
default!(default_regexlist, Regexlist);
default!(default_setup_vars, SetupVars);
default!(default_ftl_config, FtlConfig);
default!(default_local_versions, LocalVersions);
default!(default_local_branches, LocalBranches);

/// General config settings
#[derive(Deserialize)]
struct General {
    #[serde(default = "default_address")]
    address: String,
    #[serde(default = "default_port")]
    port: usize,
    #[serde(default = "default_log_level")]
    log_level: String
}

impl Default for General {
    fn default() -> Self {
        General {
            address: default_address(),
            port: default_port(),
            log_level: default_log_level()
        }
    }
}

impl General {
    fn is_valid(&self) -> bool {
        Ipv4Addr::from_str(&self.address).is_ok() && self.port <= 65535
            && match self.log_level.as_str() {
                "debug" | "normal" | "critical" => true,
                _ => false
            }
    }
}

fn default_address() -> String {
    "0.0.0.0".to_owned()
}

fn default_port() -> usize {
    80
}

fn default_log_level() -> String {
    "critical".to_owned()
}

#[cfg(test)]
mod test {
    use super::{Config, Files, General};

    #[test]
    fn valid_config() {
        let config = Config::default();
        assert!(config.is_valid());
    }

    #[test]
    fn valid_files() {
        let files = Files::default();
        assert!(files.is_valid());
    }

    #[test]
    fn valid_general() {
        let general = General::default();
        assert!(general.is_valid());
    }

    #[test]
    fn invalid_file() {
        let files = Files {
            setup_vars: "!asd?f".to_owned(),
            ..Files::default()
        };
        assert!(!files.is_valid());
    }

    #[test]
    fn invalid_general_address() {
        let general = General {
            address: "hello_world".to_owned(),
            ..General::default()
        };
        assert!(!general.is_valid());
    }

    #[test]
    fn invalid_general_port() {
        let general = General {
            port: 65536,
            ..General::default()
        };
        assert!(!general.is_valid());
    }

    #[test]
    fn invalid_general_log_level() {
        let general = General {
            log_level: "hello_world".to_owned(),
            ..General::default()
        };
        assert!(!general.is_valid());
    }
}