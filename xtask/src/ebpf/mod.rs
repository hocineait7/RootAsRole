use anyhow::Context;
use build::{build, build_ebpf, EbpfArchitecture};
use run::RunOptions;

use crate::install::BuildOptions;

pub mod build;
pub mod run;


fn clone() -> Result<(), anyhow::Error> {
    let status = std::process::Command::new("git")
        .args(&["clone", "", "capable"]).status().context("context")?;
    Ok(())
}


pub fn build_all(opts: &BuildOptions) -> Result<(), anyhow::Error> {
    build_ebpf(&opts.ebpf.unwrap_or(EbpfArchitecture::default()), &opts.profile).context("Error while building eBPF program")?;
    build(opts).context("Error while building userspace application")
}

pub fn run(opts: &RunOptions) -> Result<(), anyhow::Error> {
    build_all(&opts.build)?;
    run::run(opts)?;
    Ok(())
}