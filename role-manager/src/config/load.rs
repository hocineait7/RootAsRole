pub fn is_enforced(node: Element) -> bool {
    let enforce = node.attribute("enforce");
    (enforce.is_some()
        && enforce
            .expect("Unable to retrieve enforce attribute")
            .value()
            == "true")
        || enforce.is_none()
}

use std::{borrow::BorrowMut, cell::RefCell, error::Error, rc::Rc};

use sxd_document::dom::Element;
use tracing::warn;

use crate::{
    options::{Level, Opt},
    version::PACKAGE_VERSION,
};

use super::{
    do_in_main_element, get_groups, read_xml_file,
    structs::{IdTask, Role, Roles, Task},
};

fn get_options(level: Level, node: Element) -> Opt {
    let mut rc_options = Opt::new(level);

    for child in node.children() {
        let mut options = rc_options.borrow_mut();
        if let Some(elem) = child.element() {
            match elem.name().local_part() {
                "path" => {
                    options.path = Some(
                        elem.children()
                            .first()
                            .unwrap()
                            .text()
                            .expect("Cannot read PATH option")
                            .text()
                            .to_string(),
                    )
                }
                "env-keep" => {
                    options.env_whitelist = Some(
                        elem.children()
                            .first()
                            .unwrap()
                            .text()
                            .expect("Cannot read Whitelist option")
                            .text()
                            .to_string(),
                    )
                }
                "env-check" => {
                    options.env_checklist = Some(
                        elem.children()
                            .first()
                            .unwrap()
                            .text()
                            .expect("Cannot read Checklist option")
                            .text()
                            .to_string(),
                    )
                }
                "allow-root" => options.allow_root = Some(is_enforced(elem)),
                "allow-bounding" => options.disable_bounding = Some(is_enforced(elem)),
                "wildcard-denied" => {
                    options.wildcard_denied = Some(
                        elem.children()
                            .first()
                            .unwrap()
                            .text()
                            .expect("Cannot read Checklist option")
                            .text()
                            .to_string(),
                    )
                }
                _ => warn!("Unknown option: {}", elem.name().local_part()),
            }
        }
    }
    rc_options
}

fn get_task<'a>(
    role: &Rc<RefCell<Role<'a>>>,
    node: Element,
    i: usize,
) -> Result<Rc<RefCell<Task<'a>>>, Box<dyn Error>> {
    let task = Task::new(IdTask::Number(i), Rc::downgrade(role));
    if let Some(id) = node.attribute_value("id") {
        task.as_ref().borrow_mut().id = IdTask::Name(id.to_string());
    }
    task.as_ref().borrow_mut().capabilities =
        node.attribute_value("capabilities").map(|cap| cap.into());
    task.as_ref().borrow_mut().setuid = node.attribute_value("setuser").map(|setuid| setuid.into());
    task.as_ref().borrow_mut().setgid = node
        .attribute_value("setgroups")
        .map(|setgid| setgid.split(',').map(|e| e.to_string()).collect());
    for child in node.children() {
        if let Some(elem) = child.element() {
            match elem.name().local_part() {
                "command" => task.as_ref().borrow_mut().commands.push(
                    elem.children()
                        .first()
                        .ok_or("Unable to get text from command")?
                        .text()
                        .map(|f| f.text().to_string())
                        .ok_or("Unable to get text from command")?,
                ),
                "options" => {
                    task.as_ref().borrow_mut().options =
                        Some(Rc::new(get_options(Level::Task, elem).into()));
                }
                "purpose" => {
                    task.as_ref().borrow_mut().purpose = Some(
                        elem.children()
                            .first()
                            .ok_or("Unable to get text from purpose")?
                            .text()
                            .map(|f| f.text().to_string())
                            .ok_or("Unable to get text from purpose")?,
                    );
                }
                _ => warn!("Unknown element: {}", elem.name().local_part()),
            }
        }
    }
    Ok(task)
}

fn add_actors(role: &mut Role, node: Element) -> Result<(), Box<dyn Error>> {
    for child in node.children() {
        if let Some(elem) = child.element() {
            match elem.name().local_part() {
                "user" => role.users.push(
                    elem.attribute_value("name")
                        .ok_or("Unable to retrieve user name")?
                        .to_string(),
                ),
                "group" => role.groups.push(get_groups(elem)),
                _ => warn!("Unknown element: {}", elem.name().local_part()),
            }
        }
    }
    Ok(())
}

pub fn get_role<'a>(
    element: Element,
    roles: Option<Rc<RefCell<Roles<'a>>>>,
) -> Result<Rc<RefCell<Role<'a>>>, Box<dyn Error>> {
    let rc_role = Role::new(
        element.attribute_value("name").unwrap().to_string(),
        roles.map(|roles| Rc::downgrade(&roles)),
    );

    let mut i: usize = 0;
    for child in element.children() {
        let mut role = rc_role.as_ref().borrow_mut();
        if let Some(element) = child.element() {
            match element.name().local_part() {
                "actors" => add_actors(&mut role, element)?,
                "task" => {
                    i += 1;
                    role.tasks.push(get_task(&rc_role, element, i)?)
                }
                "options" => role.options = Some(Rc::new(get_options(Level::Role, element).into())),
                _ => warn!(
                    "Unknown element: {}",
                    child
                        .element()
                        .expect("Unable to convert unknown to element")
                        .name()
                        .local_part()
                ),
            }
        }
    }
    Ok(rc_role)
}

pub fn load_roles<'a>(filename: &str) -> Result<Rc<RefCell<Roles<'a>>>, Box<dyn Error>> {
    let package = read_xml_file(filename)?;
    let doc = package.as_document();
    let rc_roles = Roles::new(PACKAGE_VERSION);
    {
        let mut roles = rc_roles.as_ref().borrow_mut();
        do_in_main_element(doc, "rootasrole", |element| {
            if let Some(element) = element.element() {
                for role in element.children() {
                    if let Some(element) = role.element() {
                        if element.name().local_part() == "roles" {
                            for role in element.children() {
                                if let Some(element) = role.element() {
                                    if element.name().local_part() == "role" {
                                        roles
                                            .roles
                                            .push(get_role(element, Some(rc_roles.to_owned()))?);
                                    }
                                }
                            }
                        }
                        if element.name().local_part() == "options" {
                            roles.options =
                                Some(Rc::new(get_options(Level::Global, element).into()));
                        }
                    }
                }
                return Ok(());
            }
            Err("Unable to find rootasrole element".into())
        })?;
        Ok(rc_roles.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_load_roles() {
        let roles = load_roles(
            format!(
                "{}/../tests/resources/test_xml_manager_case1.xml",
                env!("PWD")
            )
            .as_str(),
        );
        if let Err(e) = roles {
            panic!("Unable to load roles: {}", e);
        }
        let binding = roles.unwrap();
        let roles = binding.as_ref().borrow();
        assert_eq!(roles.roles.len(), 2);
        let role = roles.roles.first().unwrap();
        let role = role.as_ref().borrow();
        assert_eq!(role.name, "test1");
        assert_eq!(role.users.len(), 1);
        assert_eq!(role.users.first().unwrap(), "test1");
        assert_eq!(role.groups.len(), 0);
        assert_eq!(role.tasks.len(), 2);
        let task = role.tasks.first().unwrap();
        let task = task.as_ref().borrow();
        assert_eq!(task.id, IdTask::Name("t1_test1".to_string()));
        assert_eq!(task.commands.len(), 1);
        assert_eq!(task.commands.first().unwrap(), "/bin/ls");
        let option = task.options.as_ref();
        assert!(option.is_some());
        let path = task
            .options
            .as_ref()
            .unwrap()
            .as_ref()
            .borrow()
            .to_owned()
            .path;
        assert!(path.is_some());
        assert_eq!(path.unwrap(), "t1_test1");
        assert!(task.capabilities.is_some());
        let capabilities = task.capabilities.to_owned().unwrap();
        assert_eq!(capabilities, "cap_dac_override".into());
        let task = role.tasks.last().unwrap();
        let task = task.as_ref().borrow();
        assert_eq!(task.id, IdTask::Name("t1_test2".to_string()));
        assert_eq!(task.commands.len(), 1);
        assert_eq!(task.commands.first().unwrap(), "/bin/ls");
        let option = task.options.as_ref();
        assert!(option.is_some());
        let path = task
            .options
            .as_ref()
            .unwrap()
            .as_ref()
            .borrow()
            .to_owned()
            .path;
        assert!(path.is_some());
        assert_eq!(path.unwrap(), "t1_test2");
        assert!(task.capabilities.is_none());
        let role = roles.roles.last().unwrap();
        let role = role.as_ref().borrow();
        assert_eq!(role.name, "test2");
        assert_eq!(role.users.len(), 1);
        assert_eq!(role.users.first().unwrap(), "test1");
        assert_eq!(role.groups.len(), 0);
        assert_eq!(role.tasks.len(), 1);
        let task = role.tasks.first().unwrap();
        let task = task.as_ref().borrow();
        assert_eq!(task.id, IdTask::Name("t2_test1".to_string()));
        assert_eq!(task.commands.len(), 1);
        assert_eq!(task.commands.first().unwrap(), "/bin/ls");
        let option = task.options.as_ref();
        assert!(option.is_some());
        let path = task
            .options
            .as_ref()
            .unwrap()
            .as_ref()
            .borrow()
            .to_owned()
            .path;
        assert!(path.is_some());
        assert_eq!(path.unwrap(), "t2_test1");
        let allowroot = task
            .options
            .as_ref()
            .unwrap()
            .as_ref()
            .borrow()
            .to_owned()
            .allow_root;
        assert!(allowroot.is_some());
        assert_eq!(allowroot.unwrap(), true);
        assert!(task.capabilities.is_none());
    }
}
