mod command;
mod config;
mod finder;
mod options;
mod util;
mod version;
mod timeout;

use std::{collections::HashMap, env::Vars, ops::Not, io::{stdin, stdout}, os::fd::{AsFd, RawFd, AsRawFd}};

use crate::version::PACKAGE_VERSION;
use capctl::{prctl, Cap, CapState};
use clap::Parser;
use config::{load::load_config, FILENAME};
use finder::{Cred, TaskMatcher};
use nix::{unistd::{User, getuid, Group, seteuid, setegid, setgroups, getgroups, isatty}, libc::{PATH_MAX, dev_t}, sys::stat};
use pam_client::{Context, conv_cli::Conversation, Flag};
use tracing::{debug, Level};
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser, Debug)]
#[command(name = "RootAsRole")]
#[command(author = "Eddie B. <eddie.billoir@irit.fr>")]
#[command(version = PACKAGE_VERSION)]
#[command(
    about = "Execute privileged commands with a role-based access control system",
    long_about = "sr is a tool to execute privileged commands with a role-based access control system. 
It is designed to be used in a multi-user environment, 
where users can be assigned to different roles, 
and each role has a set of rights to execute commands."
)]
struct Cli {
    /// Role to select
    #[arg(short, long)]
    role: Option<String>,
    /// Display rights of executor
    #[arg(short, long)]
    info: bool,
    /// Command to execute
    command: Vec<String>,
}

fn cap_effective(cap: Cap, enable: bool) -> Result<(), capctl::Error> {
    let mut current = CapState::get_current()?;
    current.effective.set_state(cap, enable);
    current.set_current()
}

fn setpcap_effective(enable: bool) -> Result<(), capctl::Error> {
    cap_effective(Cap::SETPCAP, enable)
}

fn setuid_effective(enable: bool) -> Result<(), capctl::Error> {
    cap_effective(Cap::SETUID, enable)
}

fn setgid_effective(enable: bool) -> Result<(), capctl::Error> {
    cap_effective(Cap::SETGID, enable)
}

fn read_effective(enable: bool) -> Result<(), capctl::Error> {
    cap_effective(Cap::DAC_READ_SEARCH, enable)
}

fn dac_override_effective(enable: bool) -> Result<(), capctl::Error> {
    cap_effective(Cap::DAC_OVERRIDE, enable)
}

fn activates_no_new_privs() -> Result<(), capctl::Error> {
    prctl::set_no_new_privs()
}

fn tz_is_safe(tzval: &str) -> bool {
    // tzcode treats a value beginning with a ':' as a path.
    let tzval = if tzval.starts_with(':') {
        &tzval[1..]
    } else {
        tzval
    };

    // Reject fully-qualified TZ that doesn't begin with the zoneinfo dir.
    if tzval.starts_with('/') {
        return false;
    }

    // Make sure TZ only contains printable non-space characters
    // and does not contain a '..' path element.
    let mut lastch = '/';
    for cp in tzval.chars() {
        if cp.is_ascii_whitespace() || !cp.is_ascii_graphic() {
            return false;
        }
        if lastch == '/'
            && cp == '.'
            && tzval
                .chars()
                .nth(tzval.chars().position(|c| c == '.').unwrap() + 1)
                == Some('.')
            && (tzval
                .chars()
                .nth(tzval.chars().position(|c| c == '.').unwrap() + 2)
                == Some('/')
                || tzval
                    .chars()
                    .nth(tzval.chars().position(|c| c == '.').unwrap() + 2)
                    == None)
        {
            return false;
        }
        lastch = cp;
    }

    // Reject extra long TZ values (even if not a path).
    if tzval.len() >= PATH_MAX.try_into().unwrap() {
        return false;
    }

    true
}

fn check_var(key: &str, value: &str) -> bool {
    if key.is_empty() || value.is_empty() {
        false
    } else {
        match key {
            "TZ" => tz_is_safe(value),
            _ => !value.contains(&['/', '%']),
        }
    }
}

fn filter_env_vars(env: Vars, checklist: &[&str], whitelist: &[&str]) -> HashMap<String, String> {
    env.filter(|(key, value)| {
        checklist.contains(&key.as_str()) && check_var(key, value)
            || whitelist.contains(&key.as_str())
    })
    .collect()
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .with_file(true)
        .with_line_number(true)
        .finish()
        .init();
    let args = Cli::parse();
    read_effective(true).expect("Failed to read_effective");
    let config = load_config(FILENAME).expect("Failed to load config file");
    read_effective(false).expect("Failed to read_effective");
    debug!("loaded config : {:#?}", config);
    let user = User::from_uid(getuid()).expect("Failed to get user").expect("Failed to get user");
    let mut groups = getgroups().expect("Failed to get groups").iter().map(|g| Group::from_gid(*g).expect("Failed to get group").expect("Failed to get group")).collect::<Vec<_>>();
    groups.insert(0,Group::from_gid(user.gid).expect("Failed to get group").expect("Failed to get group"));
    debug!(
        "User: {} ({}), Groups: {:?}",
        user.name,
        user.uid,
        groups,
    );
    let mut tty: Option<dev_t> = None;
    if let Ok(stat) =  stat::fstat(stdout().as_raw_fd()) {
        if let Ok(istty) = isatty(stdout().as_raw_fd()) {
            if istty {
                tty = Some(stat.st_rdev);
            }
        } 
    }
    // get parent pid
    let ppid = nix::unistd::getppid();

    let user = Cred { user, groups, tty, ppid  };
    
    dac_override_effective(true).expect("Failed to dac_override_effective");
    let is_valid = timeout::is_valid(&user, &user, &config.as_ref().borrow().timestamp);
    dac_override_effective(false).expect("Failed to dac_override_effective");
    debug!("need to re-authenticate : {}", !is_valid);
    if !is_valid  {
        
        let mut context = Context::new("sr", Some(&user.user.name), Conversation::new()).expect("Failed to initialize PAM");
        context.authenticate(Flag::NONE).expect("Permission Denied");
        context.acct_mgmt(Flag::NONE).expect("Permission Denied");
        timeout::add_cookie(&user, &user).expect("Failed to add cookie");
    }
    let matching = match args.role {
        None => config
            .matches(&user, &args.command)
            .expect("Permission Denied"),
        Some(role) => config
            .as_ref()
            .borrow()
            .roles
            .iter()
            .find(|r| r.as_ref().borrow().name == role)
            .expect("Permission Denied")
            .matches(&user, &args.command)
            .expect("Permission Denied"),
    };
    debug!(
        "Config : Matched user {}\n - with task {}\n - with role {}",
        user.user.name,
        matching.task().as_ref().borrow().id.to_string(),
        matching.role().as_ref().borrow().name
    );

    if args.info {
        println!("Role: {}", matching.role().as_ref().borrow().name);
        println!("Task: {}", matching.task().as_ref().borrow().id.to_string());
        println!(
            "With capabilities: {}",
            matching
                .caps()
                .unwrap_or_default()
                .into_iter()
                .fold(String::new(), |acc, cap| acc + &cap.to_string() + " ")
        );
        std::process::exit(0);
    }

    let optstack = matching.opt().as_ref().unwrap();

    // disable root
    if optstack.get_no_root().1 {
        activates_no_new_privs().expect("Failed to activate no new privs");
    }

    //setuid
    if let Some(setuid) = matching.setuid() {
        let newuser = User::from_name(setuid).expect("Failed to get user").expect("Failed to get user");
        setuid_effective(true).expect("Failed to setuid_effective");
        seteuid(newuser.uid).expect("Failed to seteuid");
    }

    //setgid
    if let Some(setgid) = matching.setgroups() {
        setgid_effective(true).expect("Failed to setgid_effective");
        let groupsid : Vec<_> = setgid
            .groups
            .iter()
            .map(|g| Group::from_name(g).expect("Failed to retrieve setgroups").expect("Failed to retrieve setgroups").gid)
            .collect();
        setegid(groupsid[0]).expect("Failed to setegid");
        setgroups(&groupsid).expect("Failed to setgroups");
    }

    //set capabilities
    if let Some(caps) = matching.caps() {
        setpcap_effective(true).expect("Failed to setpcap_effective");
        let mut capstate = CapState::empty();
        if optstack.get_bounding().1 {
            for cap in caps.not().iter() {
                capctl::bounding::drop(cap).expect("Failed to set bounding cap");
            }
        }
        capstate.permitted = caps.clone();
        capstate.inheritable = *caps;
        capstate.set_current().expect("Failed to set current cap");
        for cap in caps.iter() {
            capctl::ambient::raise(cap).expect("Failed to set ambiant cap");
        }
        setpcap_effective(false).expect("Failed to setpcap_effective");
    } else {
        setpcap_effective(true).expect("Failed to setpcap_effective");
        if optstack.get_bounding().1 {
            capctl::bounding::clear().expect("Failed to clear bounding cap");
        }
        let capstate = CapState::empty();
        capstate.set_current().expect("Failed to set current cap");
        setpcap_effective(false).expect("Failed to setpcap_effective");
    }

    //execute command
    let checklist = optstack.get_env_checklist().1;
    let whitelist = optstack.get_env_whitelist().1;
    let veccheck: Vec<&str> = checklist.split(',').collect();
    let vecwhitelist: Vec<&str> = whitelist.split(',').collect();
    let mut command = std::process::Command::new(&matching.file_exec_path())
        .args(matching.exec_args())
        .envs(filter_env_vars(std::env::vars(), &veccheck, &vecwhitelist))
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .expect("Failed to execute command");
    //wait for command to finish
    let status = command.wait().expect("Failed to wait for command");
    std::process::exit(status.code().unwrap_or(1));
}
