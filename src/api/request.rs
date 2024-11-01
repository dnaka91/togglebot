use super::{AdminId, Source};

#[cfg_attr(test, derive(PartialEq))]
pub enum Request {
    User(User),
    Admin(Admin),
    Owner(Owner),
}

#[cfg_attr(test, derive(PartialEq))]
pub enum User {
    Help,
    Commands(Source),
    Links,
    Ban(String),
    Crate(String),
    Today,
    Ftoc(f64),
    Ctof(f64),
    Custom(String),
}

#[cfg_attr(test, derive(PartialEq))]
pub enum Admin {
    Help,
    CustomCommands(CustomCommands),
    Statistics(StatisticsDate),
}

#[cfg_attr(test, derive(PartialEq))]
pub enum CustomCommands {
    List,
    Add {
        source: Option<Source>,
        name: String,
        content: String,
    },
    Remove {
        source: Option<Source>,
        name: String,
    },
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum StatisticsDate {
    Total,
    Current,
}

#[cfg_attr(test, derive(PartialEq))]
pub enum Owner {
    Help,
    Admins(Admins),
}

#[cfg_attr(test, derive(PartialEq))]
pub enum Admins {
    List,
    Add(AdminId),
    Remove(AdminId),
}
