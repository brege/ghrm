use anyhow::{Result, bail};
use std::path::Path;
use std::process::{Command, Stdio};

pub fn output(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("--no-pager")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()?;

    if !output.status.success() {
        bail!("git command failed");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn global_output(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()?;

    if !output.status.success() {
        bail!("git command failed");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
