use std::cell::RefCell;
use std::collections::HashSet;
use std::error::Error;
use std::hash::{Hash, Hasher};
use std::rc::{Rc, Weak};
use std::str::Split;

use sxd_document::dom::{Document, Element};

use crate::capabilities::Caps;
use crate::options::Opt;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Groups {
    pub groups: HashSet<String>,
}

impl Iterator for Groups {
    fn next(&mut self) -> Option<String> {
        self.groups.iter().next().map(|s| s.to_owned())
    }
    type Item = String;
}

impl Hash for Groups {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for group in self.groups.iter() {
            group.hash(state);
        }
    }
}

impl FromIterator<String> for Groups {
    fn from_iter<T: IntoIterator<Item = String>>(iter: T) -> Groups {
        let mut groups = HashSet::new();
        for group in iter {
            groups.insert(group);
        }
        Groups { groups }
    }
}

impl From<Vec<String>> for Groups {
    fn from(groups: Vec<std::string::String>) -> Self {
        let mut set = HashSet::new();
        for group in groups {
            set.insert(group);
        }
        Groups { groups: set }
    }
}

impl From<Split<'_, char>> for Groups {
    fn from(groups: Split<char>) -> Self {
        let mut set = HashSet::new();
        for group in groups {
            set.insert(group.to_owned());
        }
        Groups { groups: set }
    }
}

impl Groups {
    pub fn join(&self, sep: &str) -> String {
        self.groups.iter().fold(String::new(), |acc, s| {
            if acc.is_empty() {
                s.to_owned()
            } else {
                format!("{}{}{}", acc, sep, s)
            }
        })
    }
}

impl From<Groups> for Vec<String> {
    fn from(val: Groups) -> Self {
        val.groups.into_iter().collect()
    }
}

pub trait ToXml {
    fn to_xml_string(&self) -> String;
}

#[derive(Clone, Debug)]
pub enum IdTask {
    Name(String),
    Number(usize),
}

impl IdTask {
    pub fn is_name(&self) -> bool {
        match self {
            IdTask::Name(_) => true,
            IdTask::Number(_) => false,
        }
    }

    pub fn as_ref(&self) -> &IdTask {
        self
    }

    pub fn unwrap(&self) -> String {
        match self {
            IdTask::Name(s) => s.to_owned(),
            IdTask::Number(s) => s.to_string(),
        }
    }
}

impl ToString for IdTask {
    fn to_string(&self) -> String {
        match self {
            IdTask::Name(s) => s.to_string(),
            IdTask::Number(n) => format!("Task #{}", n),
        }
    }
}

impl From<String> for IdTask {
    fn from(s: String) -> Self {
        IdTask::Name(s)
    }
}

impl From<IdTask> for String {
    fn from(val: IdTask) -> Self {
        match val {
            IdTask::Name(s) => s,
            IdTask::Number(n) => n.to_string(),
        }
    }
}

impl PartialEq for IdTask {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (IdTask::Name(a), IdTask::Name(b)) => a == b,
            (IdTask::Number(a), IdTask::Number(b)) => a == b,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Task<'a> {
    role: Weak<RefCell<Role<'a>>>,
    pub id: IdTask,
    pub options: Option<Rc<RefCell<Opt>>>,
    pub commands: Vec<String>,
    pub capabilities: Option<Caps>,
    pub setuid: Option<String>,
    pub setgid: Option<Groups>,
    pub purpose: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Role<'a> {
    roles: Option<Weak<RefCell<Roles<'a>>>>,
    pub name: String,
    pub users: Vec<String>,
    pub groups: Vec<Groups>,
    pub tasks: Vec<Rc<RefCell<Task<'a>>>>,
    pub options: Option<Rc<RefCell<Opt>>>,
}

#[derive(Debug, Clone)]
pub struct Roles<'a> {
    pub roles: Vec<Rc<RefCell<Role<'a>>>>,
    pub options: Option<Rc<RefCell<Opt>>>,
    pub version: &'a str,
}

impl<'a> Roles<'a> {
    pub fn new(version: &str) -> Rc<RefCell<Roles>> {
        Rc::new(
            Roles {
                roles: Vec::new(),
                options: None,
                version,
            }
            .into(),
        )
    }

    pub fn get_role(&self, name: &str) -> Option<Rc<RefCell<Role<'a>>>> {
        for r in self.roles.iter() {
            if r.as_ref().borrow().name == name {
                return Some(r.to_owned());
            }
        }
        None
    }

    pub fn get_roles_names(&self) -> HashSet<String> {
        let mut set = HashSet::new();
        for r in self.roles.iter() {
            set.insert(r.as_ref().borrow().name.to_string());
        }
        set
    }
}

impl<'a> Role<'a> {
    pub fn new(name: String, roles: Option<Weak<RefCell<Roles<'a>>>>) -> Rc<RefCell<Role<'a>>> {
        Rc::new(
            Role {
                roles,
                name,
                users: Vec::new(),
                groups: Vec::new(),
                tasks: Vec::new(),
                options: None,
            }
            .into(),
        )
    }
    pub fn get_task_from_index(&self, index: &usize) -> Option<Rc<RefCell<Task<'a>>>> {
        if self.tasks.len() > *index {
            return Some(self.tasks[*index].to_owned());
        }
        None
    }
    pub fn get_users_info(&self) -> String {
        let mut users_info = String::new();
        users_info.push_str(&format!("Users:\n({})\n", self.users.to_vec().join(", ")));
        users_info
    }
    pub fn get_groups_info(&self) -> String {
        let mut groups_info = String::new();
        groups_info.push_str(&format!(
            "Groups:\n({})\n",
            self.groups
                .iter()
                .cloned()
                .map(|x| x.join(" & "))
                .collect::<Vec<String>>()
                .join(")\n(")
        ));
        groups_info
    }
    pub fn get_tasks_info(&self) -> String {
        let mut tasks_info = String::new();
        tasks_info.push_str(&format!(
            "Tasks:\n{}\n",
            self.tasks
                .iter()
                .cloned()
                .map(|x| x.as_ref().borrow().id.to_string())
                .collect::<Vec<String>>()
                .join("\n")
        ));
        tasks_info
    }
    pub fn get_options_info(&self) -> String {
        let mut options_info = String::new();
        if let Some(o) = &self.options {
            options_info.push_str(&format!(
                "Options:\n{}",
                o.as_ref().borrow().get_description()
            ));
        }
        options_info
    }

    pub fn get_description(&self) -> String {
        let mut description = String::new();
        description.push_str(&self.get_users_info());
        description.push_str(&self.get_groups_info());
        description.push_str(&self.get_tasks_info());
        description.push_str(&self.get_options_info());
        description
    }

    pub fn remove_task(&mut self, id: IdTask) {
        let mut tasks = self.tasks.to_owned();
        tasks.retain(|x| x.as_ref().borrow().id != id);
        self.tasks = tasks;
    }
}

impl<'a> Task<'a> {
    pub fn new(id: IdTask, role: Weak<RefCell<Role<'a>>>) -> Rc<RefCell<Task<'a>>> {
        Rc::new(
            Task {
                role,
                id,
                options: None,
                commands: Vec::new(),
                capabilities: None,
                setuid: None,
                setgid: None,
                purpose: None,
            }
            .into(),
        )
    }
    pub fn get_parent(&self) -> Option<Rc<RefCell<Role<'a>>>> {
        self.role.upgrade()
    }

    pub fn get_description(&self) -> String {
        let mut description = String::new();

        if let Some(p) = &self.purpose {
            description.push_str(&format!("Purpose :\n{}\n", p));
        }

        if let Some(caps) = self.capabilities.to_owned() {
            description.push_str(&format!("Capabilities:\n({})\n", caps.to_string()));
        }
        if let Some(setuid) = self.setuid.to_owned() {
            description.push_str(&format!("Setuid:\n({})\n", setuid));
        }
        if let Some(setgid) = self.setgid.to_owned() {
            description.push_str(&format!("Setgid:\n({})\n", setgid.join(" & ")));
        }

        if let Some(options) = self.options.to_owned() {
            description.push_str(&format!(
                "Options:\n({})\n",
                options.as_ref().borrow().get_description()
            ));
        }

        description.push_str(&format!(
            "Commands:\n{}\n",
            self.commands
                .iter()
                .map(|s| {
                    if s.len() < 64 {
                        s.to_owned()
                    } else {
                        let mut s = s.to_owned().chars().take(64).collect::<String>();
                        s.push_str("...");
                        s
                    }
                })
                .fold(String::new(), |acc, x| acc + &format!("{}\n", x))
        ));
        description
    }
}

pub trait Save {
    fn save(
        &self,
        doc: Option<&Document>,
        element: Option<&Element>,
    ) -> Result<bool, Box<dyn Error>>;
}

#[cfg(test)]
mod tests {

    use crate::options::Level;

    use super::*;

    #[test]
    fn test_get_empty_description() {
        let role = Role::new("test_role".to_string(), None);
        assert_eq!(
            role.as_ref().borrow().get_description(),
            "Users:\n()\nGroups:\n()\nTasks:\n\n"
        );
        let task = Task::new(IdTask::Number(0), Rc::downgrade(&role));
        assert_eq!(task.as_ref().borrow().get_description(), "Commands:\n\n");
    }

    #[test]
    fn test_get_description() {
        let role = Role::new("test_role".to_string(), None);
        let task = Task::new(IdTask::Number(0), Rc::downgrade(&role));
        task.as_ref().borrow_mut().commands.push("ls".to_string());
        task.as_ref()
            .borrow_mut()
            .commands
            .push("another".to_string());
        task.as_ref().borrow_mut().purpose = Some("thepurpose".to_string());
        task.as_ref().borrow_mut().setuid = Some("thesetuid".to_string());
        task.as_ref().borrow_mut().setgid =
            Some(vec!["thesetgid".to_string(), "thesecondsetgid".to_string()].into());
        task.as_ref().borrow_mut().capabilities = Some(Caps::V2(3));
        let mut opt = Opt::new(Level::Task);
        opt.path = Some("thepath".to_string());
        opt.disable_bounding = Some(false);
        opt.allow_root = Some(true);
        opt.wildcard_denied = Some("thewildcard-denied".to_string());
        opt.env_checklist = Some("thechecklist".to_string());
        opt.env_whitelist = Some("thewhitelist".to_string());
        task.as_ref().borrow_mut().options = Some(Rc::new(RefCell::new(opt)));
        let desc = task.as_ref().borrow().get_description();
        println!("{}", desc);
        assert!(desc.contains("ls\nanother\n"));
        assert!(desc.contains("thepurpose"));
        assert!(desc.contains("thesetuid"));
        assert!(desc.contains("thesetgid"));
        assert!(desc.contains("thesecondsetgid"));
        assert!(desc.contains(&Caps::V2(3).to_string()));
        assert!(desc.contains("Options"));
        assert!(desc.contains("thepath"));
        assert!(desc.contains("thewildcard-denied"));
        assert!(desc.contains("thechecklist"));
        assert!(desc.contains("thewhitelist"));
        assert!(desc.contains("No root: true"));
        assert!(desc.contains("Bounding: false"));
    }

    #[test]
    fn test_idtask() {
        let id = IdTask::Number(0);
        assert_eq!(id.to_string(), "Task #0");
        let id = IdTask::Name("test".to_string());
        assert_eq!(id.to_string(), "test");
        let id: IdTask = "test".to_string().into();
        assert_eq!(Into::<String>::into(id), "test");
    }
}
