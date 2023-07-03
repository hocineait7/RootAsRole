use std::{
    borrow::{BorrowMut},
    cell::RefCell,
    error::Error,
    rc::Rc,
};

use sxd_document::{
    dom::{Document, Element},
};
use sxd_xpath::{Context, Factory, Value};
use tracing::warn;


use crate::{
    options::{Level, Opt},
    version::PACKAGE_VERSION,
    config::{Role, Roles, Task, IdTask, is_enforced, get_groups, read_xml_file},
};

pub fn find_role<'a>(
    doc: &'a Document,
    name: &'a str,
) -> Result<Rc<RefCell<Role<'a>>>, Box<dyn Error>> {
    let factory = Factory::new();
    let context = Context::new();
    let xpath = factory.build(&format!("//role[@name='{}']", name))?;
    let value = xpath.unwrap().evaluate(&context, doc.root())?;
    if let Value::Nodeset(nodes) = value {
        if nodes.size() != 0 {
            let role_element = nodes
                .iter()
                .next()
                .expect("Unable to retrieve element")
                .element()
                .expect("Unable to convert role node to element");
            return get_role(role_element, None);
        }
    }
    Err("Role not found".into())
}

fn get_options(level: Level, node: Element) -> Opt {
    let mut rc_options = Opt::new(level);

    for child in node.children() {
        let mut options = rc_options.borrow_mut();
        if let Some(elem) = child.element() {
            println!("{}", elem.name().local_part());
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
                    .into()
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
                    .into()
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
                    .into()
                }
                "allow-root" => options.no_root = Some(is_enforced(elem)).into(),
                "allow-bounding" => options.bounding = Some(is_enforced(elem)).into(),
                "wildcard-denied" => options.wildcard_denied = Some(
                    elem.children()
                        .first()
                        .unwrap()
                        .text()
                        .expect("Cannot read Checklist option")
                        .text()
                        .to_string()),
                _ => warn!("Unknown option: {}", elem.name().local_part()),
            }
        }
    }
    rc_options
}

fn get_task<'a>(role: &Rc<RefCell<Role<'a>>>, node: Element, i: usize) -> Result<Rc<RefCell<Task<'a>>>, Box<dyn Error>> {
    let task = Task::new(IdTask::Number(i), Rc::downgrade(role));
    if let Some(id) = node.attribute_value("id") {
        task.as_ref().borrow_mut().id = IdTask::Name(id.to_string());
    }
    task.as_ref().borrow_mut().capabilities = match node.attribute_value("capabilities") {
        Some(cap) => Some(cap.into()).into(),
        None => None.into(),
    };
    task.as_ref().borrow_mut().setuid = match node.attribute_value("setuser") {
        Some(setuid) => Some(setuid.into()).into(),
        None => None.into(),
    };
    task.as_ref().borrow_mut().setgid = match node.attribute_value("setgroups") {
        Some(setgid) => Some(setgid.split(",").map(|e| e.to_string()).collect()).into(),
        None => None.into(),
    };
    for child in node.children() {
        if let Some(elem) = child.element() {
            match elem.name().local_part() {
                "command" => task.as_ref().borrow_mut().commands.push(
                    elem.children()
                        .first()
                        .ok_or("Unable to get text from command")?
                        .text()
                        .map(|f| f.text().to_string())
                        .ok_or("Unable to get text from command")?
                        .into(),
                ),
                "options" => {
                    task.as_ref().borrow_mut().options = Some(Rc::new(get_options(Level::Task, elem).into()));
                },
                "purpose" => {
                    task.as_ref().borrow_mut().purpose = Some(
                        elem.children()
                            .first()
                            .ok_or("Unable to get text from purpose")?
                            .text()
                            .map(|f| f.text().to_string())
                            .ok_or("Unable to get text from purpose")?
                            .into(),
                    );
                }
                _ => warn!("Unknown element: {}", elem.name().local_part()),
            }
        }
    }
    Ok(task)
}

fn add_actors(role : &mut Role, node: Element) -> Result<(), Box<dyn Error>> {
    for child in node.children() {
        if let Some(elem) = child.element() {
            println!("{}", elem.name().local_part());
            match elem.name().local_part() {
                "user" => role.users.push(
                    elem
                        .attribute_value("name")
                        .ok_or("Unable to retrieve user name")?
                        .to_string()
                        .into(),
                ),
                "group" => role.groups.push(get_groups(elem)),
                _ => warn!("Unknown element: {}", elem.name().local_part()),
            }
        }
    }
    Ok(())
}

pub fn get_role<'a>(element: Element, roles: Option<Rc<RefCell<Roles<'a>>>>) -> Result<Rc<RefCell<Role<'a>>>, Box<dyn Error>> {
    let rc_role = Role::new(
        element.attribute_value("name").unwrap().to_string().into(),
        match roles {
            Some(roles) => Some(Rc::downgrade(&roles)),
            None => None,
        }
    );
    
    let mut i: usize = 0;
    for child in element.children() {
        let mut role = rc_role.as_ref().borrow_mut();
        if let Some(element) = child.element() {
            match element.name().local_part() {
                "actors" => add_actors(&mut role, element)?,
                "task" => {
                    i += 1;
                    role.tasks
                        .push(get_task(&rc_role, element, i)?)
                }
                "options" => {
                    role.options = Some(Rc::new(get_options(Level::Role, element).into())).into()
                }
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

pub fn load_roles<'a>(filename : &str) -> Result<Rc<RefCell<Roles<'a>>>, Box<dyn Error>> {
    let package = read_xml_file(filename).expect("Failed to read xml file");
    let doc = package.as_document();
    let rc_roles = Roles::new(PACKAGE_VERSION);
    {
    let mut roles = rc_roles.as_ref().borrow_mut();
    for child in doc.root().children() {
        if let Some(element) = child.element() {
            if element.name().local_part() == "rootasrole" {
               
                for role in element.children() {
                    if let Some(element) = role.element() {
                        if element.name().local_part() == "roles" {
                            for role in element.children() {
                                if let Some(element) = role.element() {
                                    if element.name().local_part() == "role" {
                                        roles.roles.push(
                                            get_role(element,Some(rc_roles.to_owned()))?);
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
                return Ok(rc_roles.to_owned());
            }
        }
    }
}
Err("Unable to find rootasrole element".into())
}