use std::{collections::HashSet, error::Error};

use clap::{Parser, Subcommand};

use crate::{
    capabilities::Caps,
    config::structs::{Groups, IdTask, Save},
    options::{OptType, OptValue},
    rolemanager::RoleContext,
    version::PACKAGE_VERSION,
};

//rar newrole "role1" --user "user1" --group "group1" "group2"
//rar addtask "role1" --cmds "command1" --caps "cap_dac_override,cap_dac_read_search"
//rar addtask "role1" --with-id "myid" --cmds "command2" --caps "cap_dac_override"

//rar deltask "role1" "myid"

//rar grant "role1" --user "user1" --group "group1,group2"
//rar revoke "role1" --user "user1"

//rar delrole "role1"

//rar config --role "role1" --task "myid" --path "/usr/bin:/bin"
//rar config --role "role1" --env "MYVAR=1"
//rar config --allow-bounding false

#[derive(Parser, Debug)]
#[command(name = "RootAsRole")]
#[command(author = "Eddie B. <eddie.billoir@irit.fr>")]
#[command(version = PACKAGE_VERSION)]
#[command(
    about = "Configure Roles for RootAsRole",
    long_about = "Role Manager is a tool to configure RBAC for RootAsRole.
A role is a set of tasks that can be executed by a user or a group of users.
These tasks are multiple commands associated with their permissions (capabilities).
Like Sudo, you could manipulate environment variables, PATH, and other options.
But Sudo is not designed to use permissions for commands."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<CCommand>,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
enum CCommand {
    /// List all roles
    #[command(name = "list")]
    List {
        #[arg(short, long)]
        /// Describe role
        role: Option<String>,
        #[arg(short, long)]
        /// Describe task within role
        task: Option<String>,
    },
    /// Create a new role, you can add users, groups, tasks. You can assign tasks through the command "addtask"
    #[command(name = "newrole")]
    NewRole {
        role: String,
        #[arg(short, long)]
        user: Option<Vec<String>>,
        #[arg(short, long)]
        group: Option<Vec<String>>,
    },
    /// You can grant users/groups to role
    #[command(name = "grant")]
    Grant {
        role: String,
        #[arg(short, long)]
        user: Option<Vec<String>>,
        #[arg(short, long)]
        group: Option<Vec<String>>,
    },
    /// You can revoke users/groups from role
    #[command(name = "revoke")]
    Revoke {
        role: String,
        #[arg(short, long)]
        user: Option<Vec<String>>,
        #[arg(short, long)]
        group: Option<Vec<String>>,
    },
    /// Add a task to a role, you can add commands and capabilities
    #[command(name = "addtask")]
    AddTask {
        role: String,
        #[arg(short, long)]
        withid: Option<String>,
        #[arg(short, long)]
        cmds: Option<Vec<String>>,
        #[arg(short = 'p', long)]
        caps: Option<String>,
    },
    /// Delete a task from a role
    #[command(name = "deltask")]
    DelTask { role: String, id: String },
    /// Delete a role, this is not reversible
    #[command(name = "delrole")]
    DelRole { role: String },
    /// You could configure options for all roles, specific role, or specific task
    #[command(name = "config")]
    Config {
        #[arg(short, long)]
        /// Role name
        role: Option<String>,
        #[arg(short, long)]
        /// Task id or index in the list
        task: Option<String>,
        /// Set PATH environment variable
        #[arg(long)]
        path: Option<String>,
        /// Keep environment variables without changing them
        #[arg(long)]
        env_keep: Option<String>,
        /// Keep environment variables if they are valid
        #[arg(long)]
        env_check: Option<String>,
        /// When false, capabilties are permanently dropped, when true, process can regain them (with sudo as example)
        #[arg(long)]
        allow_bounding: Option<bool>,
        /// When you configure command with wildcard, you can except chars of wildcard match
        #[arg(long)]
        wildcard_denied: Option<String>,
    },
    /// NOT IMPLEMENTED: Import sudoers file
    Import {
        /// Import sudoers file as RootAsRole roles
        file: String,
    },
}

/**
 * Parse the command line arguments
 */
pub fn parse_args(manager: &mut RoleContext) -> Result<bool, Box<dyn Error>> {
    let args = Cli::parse();
    match args.command.as_ref() {
        Some(CCommand::NewRole { role, user, group }) => {
            manager.create_new_role(role.to_owned());
            let role = manager.get_role().unwrap();
            if let Some(user) = user.to_owned() {
                let user = user
                    .into_iter()
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                role.as_ref().borrow_mut().users = user.to_owned();
            }
            if let Some(group) = group.to_owned() {
                let group = group
                    .into_iter()
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect::<Vec<String>>();
                role.as_ref().borrow_mut().groups = group
                    .iter()
                    .map(|x| Into::<Groups>::into(x.split(',')))
                    .collect::<Vec<Groups>>();
            }
            manager.save_new_role();
            manager.save(None, None)?;
            Ok(true)
        }
        Some(CCommand::Grant { role, user, group }) => {
            let mut res = false;
            if let Some(role) = manager.find_role(&role) {
                if let Some(user) = user.to_owned() {
                    for u in user {
                        if !role.as_ref().borrow().users.contains(&u) {
                            role.as_ref().borrow_mut().users.push(u);
                        }
                    }
                    res = true;
                }
                if let Some(group) = group.to_owned() {
                    role.as_ref().borrow_mut().groups.append(
                        &mut group
                            .iter()
                            .map(|x| Into::<Groups>::into(x.split('&')))
                            .collect::<Vec<Groups>>(),
                    );
                    role.as_ref().borrow_mut().groups = role
                        .as_ref()
                        .borrow_mut()
                        .groups
                        .clone()
                        .into_iter()
                        .collect::<HashSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>();
                    res = true;
                }
                if res {
                    manager.save(None, None)?;
                }
            }
            Ok(res)
        }
        Some(CCommand::Revoke { role, user, group }) => {
            let mut res = false;
            if let Some(role) = manager.find_role(&role) {
                if let Some(user) = user.to_owned() {
                    for u in user {
                        if !role.as_ref().borrow().users.contains(&u) {
                            role.as_ref().borrow_mut().users.retain(|x| x != &u);
                        }
                    }
                    res = true;
                }
                if let Some(group) = group.to_owned() {
                    role.as_ref().borrow_mut().groups = group
                        .iter()
                        .map(|x| Into::<Groups>::into(x.split('&')))
                        .collect::<Vec<Groups>>();
                    res = true;
                }
                if res {
                    manager.save(None, None)?;
                }
            }
            Ok(res)
        }
        Some(CCommand::AddTask {
            role,
            withid,
            cmds,
            caps,
        }) => {
            manager.select_role_by_name(&role)?;
            manager.create_new_task(withid.to_owned())?;
            let task = manager.get_task().unwrap();
            if let Some(cmds) = cmds.to_owned() {
                task.as_ref().borrow_mut().commands = cmds;
            }
            if let Some(caps) = caps.to_owned() {
                task.as_ref().borrow_mut().capabilities = Some(Caps::from(caps));
            }
            manager.save(None, None)?;
            Ok(true)
        }
        Some(CCommand::DelTask { role, id }) => {
            manager.select_role_by_name(&role)?;
            manager.select_task_by_id(&IdTask::Name(id.to_owned()))?;
            manager.delete_task()?;
            manager.save(None, None)?;
            Ok(true)
        }
        Some(CCommand::DelRole { role }) => {
            manager.select_role_by_name(&role)?;
            manager.delete_role()?;
            manager.save(None, None)?;
            Ok(true)
        }
        Some(CCommand::Config {
            role,
            task,
            path,
            env_keep,
            env_check,
            allow_bounding,
            wildcard_denied,
        }) => {
            if let Some(role) = role {
                manager.select_role_by_name(&role)?;
            }
            if let Some(task) = task {
                let tid = match task.parse::<usize>() {
                    Ok(id) => IdTask::Number(id),
                    Err(_) => IdTask::Name(task.to_owned()),
                };
                manager.select_task_by_id(&tid)?;
            }
            if let Some(path) = path {
                manager
                    .get_options()
                    .set_value(OptType::Path, Some(OptValue::String(path.to_owned())));
            }
            if let Some(env_keep) = env_keep {
                manager.get_options().set_value(
                    OptType::EnvWhitelist,
                    Some(OptValue::String(env_keep.to_owned())),
                );
            }
            if let Some(env_check) = env_check {
                manager.get_options().set_value(
                    OptType::EnvChecklist,
                    Some(OptValue::String(env_check.to_owned())),
                );
            }
            if let Some(allow_bounding) = allow_bounding {
                manager.get_options().set_value(
                    OptType::Bounding,
                    Some(OptValue::Bool(allow_bounding.to_owned())),
                );
            }
            if let Some(wildcard_denied) = wildcard_denied {
                manager.get_options().set_value(
                    OptType::Wildcard,
                    Some(OptValue::String(wildcard_denied.to_owned())),
                );
            }
            manager.save(None, None)?;
            Ok(true)
        }
        Some(CCommand::List { role, task }) => {
            if let Some(role) = role {
                manager.select_role_by_name(&role)?;
                if let Some(task) = task {
                    let tid = match task.parse::<usize>() {
                        Ok(id) => IdTask::Number(id),
                        Err(_) => IdTask::Name(task.to_owned()),
                    };
                    manager.select_task_by_id(&tid)?;
                    let task = manager.get_task().unwrap();
                    println!("{}", task.as_ref().borrow().get_description());
                } else {
                    let role = manager.get_role().unwrap();
                    println!("{}", role.as_ref().borrow().get_description());
                }
            }
            Ok(true)
        }
        Some(CCommand::Import { file: _ }) => Err("not implemented".into()),
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse_args_new_role() {
        let args = Cli::parse_from(&[
            "rar", "newrole", "admin", "--user", "user1", "--group", "group1",
        ])
        .command;
        let expected_command = Some(CCommand::NewRole {
            role: "admin".to_string(),
            user: Some(["user1".to_string()].to_vec()),
            group: Some(["group1".to_string()].to_vec()),
        });
        assert_eq!(args, expected_command);
    }

    #[test]
    fn test_parse_args_grant() {
        let args = Cli::parse_from(&[
            "rar", "grant", "admin", "--user", "user1", "--group", "group1",
        ])
        .command;
        let expected_command = Some(CCommand::Grant {
            role: "admin".to_string(),
            user: Some(["user1".to_string()].to_vec()),
            group: Some(["group1".to_string()].to_vec()),
        });
        assert_eq!(args, expected_command);
    }

    #[test]
    fn test_parse_args_revoke() {
        let args = Cli::parse_from(&[
            "rar", "revoke", "admin", "--user", "user1", "--group", "group1",
        ])
        .command;
        let expected_command = Some(CCommand::Revoke {
            role: "admin".to_string(),
            user: Some(["user1".to_string()].to_vec()),
            group: Some(["group1".to_string()].to_vec()),
        });
        assert_eq!(args, expected_command);
    }

    #[test]
    fn test_parse_args_add_task() {
        let args = Cli::parse_from(&[
            "rar", "addtask", "admin", "--withid", "task1", "--cmds", "cmd1", "--caps", "cap1",
        ])
        .command;
        let expected_command = Some(CCommand::AddTask {
            role: "admin".to_string(),
            withid: Some("task1".to_string()),
            cmds: Some(["cmd1".to_string()].to_vec()),
            caps: Some("cap1".to_string()),
        });
        assert_eq!(args, expected_command);
    }

    #[test]
    fn test_parse_args_del_task() {
        let args = Cli::parse_from(&["rar", "deltask", "admin", "task1"]).command;
        let expected_command = Some(CCommand::DelTask {
            role: "admin".to_string(),
            id: "task1".to_string(),
        });
        assert_eq!(args, expected_command);
    }

    #[test]
    fn test_parse_args_del_role() {
        let args = Cli::parse_from(&["rar", "delrole", "admin"]).command;
        let expected_command = Some(CCommand::DelRole {
            role: "admin".to_string(),
        });
        assert_eq!(args, expected_command);
    }

    #[test]
    fn test_parse_args_config() {
        let args = Cli::parse_from(&[
            "rar",
            "config",
            "--role",
            "admin",
            "--task",
            "task1",
            "--path",
            "/path/to/file",
            "--env-keep",
            "env1",
            "--env-check",
            "env2",
            "--allow-bounding",
            "true",
            "--wildcard-denied",
            "false",
        ])
        .command;
        let expected_command = Some(CCommand::Config {
            role: Some("admin".to_string()),
            task: Some("task1".to_string()),
            path: Some("/path/to/file".to_string()),
            env_keep: Some("env1".to_string()),
            env_check: Some("env2".to_string()),
            allow_bounding: Some(true),
            wildcard_denied: Some(";;".to_string()),
        });
        assert_eq!(args, expected_command);
    }

    #[test]
    fn test_parse_args_list() {
        let args = Cli::parse_from(&["rar", "list", "--role", "admin", "--task", "task1"]).command;
        let expected_command = Some(CCommand::List {
            role: Some("admin".to_string()),
            task: Some("task1".to_string()),
        });
        assert_eq!(args, expected_command);
    }

    #[test]
    fn test_parse_args_import() {
        let args = Cli::parse_from(&["rar", "import", "/path/to/file"]).command;
        let expected_command = Some(CCommand::Import {
            file: "/path/to/file".to_string(),
        });
        assert_eq!(args, expected_command);
    }

    #[test]
    fn test_parse_args_no_command() {
        let args = Cli::parse_from(&["rar"]).command;
        let expected_command = None;
        assert_eq!(args, expected_command);
    }
}
