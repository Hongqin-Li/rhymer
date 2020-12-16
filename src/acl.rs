#[derive(PartialEq, Debug)]
pub enum AclType {
    None,
    Invisible,
    ReadOnly,
    ReadWrite,
}
impl Default for AclType {
    fn default() -> Self {
        AclType::None
    }
}

#[derive(Debug, Default)]
pub struct AclItem {
    id: String,
    acl: AclType,
}

#[derive(Debug, Default)]
pub struct Acl {
    all: AclType,
    users: Vec<AclItem>,
}

impl Acl {
    pub fn all_readable(&self) -> bool {
        if self.all == AclType::ReadOnly || self.all == AclType::ReadWrite {
            true
        } else {
            false
        }
    }
    pub fn all_writable(&self) -> bool {
        if self.all == AclType::ReadWrite {
            true
        } else {
            false
        }
    }
    pub fn all_invisible(&self) -> bool {
        if self.all == AclType::Invisible {
            true
        } else {
            false
        }
    }
    pub fn not_all(&self) -> bool {
        if self.all == AclType::None {
            true
        } else {
            false
        }
    }

    pub fn get_readers(&self) -> Vec<String> {
        self.users
            .iter()
            .filter_map(|x| {
                if x.acl == AclType::ReadOnly || x.acl == AclType::ReadWrite {
                    Some(x.id.clone())
                } else {
                    None
                }
            })
            .collect()
    }
    pub fn get_writers(&self) -> Vec<String> {
        self.users
            .iter()
            .filter_map(|x| {
                if x.acl == AclType::ReadWrite {
                    Some(x.id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn readable(&self, uid: &str) -> bool {
        //self.ge
        todo!()
    }
    pub fn writable(&self, uid: &str) -> bool {
        todo!()
    }
}
