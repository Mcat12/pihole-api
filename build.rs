// Pi-hole: A black hole for Internet advertisements
// (c) 2019 Pi-hole, LLC (https://pi-hole.net)
// Network-wide ad blocking via your own hardware.
//
// API
// Build Script For Retrieving VCS Data
//
// This file is copyright under the latest version of the EUPL.
// Please see LICENSE file for your rights under this license.

use std::{env, process::Command};

// Read Git data and expose it to the API at compile time
fn main() {
    // Use the CIRCLE_TAG variable if it exists
    let tag = env::var("CIRCLE_TAG").unwrap_or_else(|_| {
        // Otherwise read from Git
        let tag_raw = Command::new("git")
            .args(&["describe", "--tags", "--abbrev=0", "--exact-match"])
            .output()
            .map(|output| output.stdout)
            .unwrap_or_default();
        String::from_utf8(tag_raw).unwrap()
    });

    // Use the CIRCLE_BRANCH variable if it exists
    let branch = env::var("CIRCLE_BRANCH").unwrap_or_else(|_| {
        // Otherwise read from Git
        let branch_raw = Command::new("git")
            .args(&["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .map(|output| output.stdout)
            .unwrap_or_default();
        let branch = String::from_utf8(branch_raw).unwrap();

        // Check if this is a tag build
        if !tag.is_empty() && branch == "HEAD" {
            // Tag builds should have a branch of master
            "master".to_owned()
        } else {
            branch
        }
    });

    let hash_raw = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .map(|output| output.stdout)
        .unwrap_or_default();
    let hash = String::from_utf8(hash_raw).unwrap();

    let version = if !tag.is_empty() {
        tag.to_owned()
    } else {
        format!("vDev-{}", hash.trim().get(0..7).unwrap_or_default())
    };

    // This lets us use the `env!()` macro to read these variables at compile time
    println!("cargo:rustc-env=GIT_TAG={}", tag.trim());
    println!("cargo:rustc-env=GIT_BRANCH={}", branch.trim());
    println!("cargo:rustc-env=GIT_HASH={}", hash.trim());
    println!("cargo:rustc-env=GIT_VERSION={}", version);
}
