use std::num::NonZeroU64;

use anyhow::Result;

use crate::api::{
    request::{self, Request, StatisticsDate},
    Source,
};

macro_rules! bail {
    ($e:expr $(,)?) => {
        return Some(Err(anyhow::anyhow!($e)))
    };
}

macro_rules! err {
    ($f:expr) => {
        match $f {
            Ok(v) => v,
            Err(e) => return Some(Err(e.into())),
        }
    };
}

pub fn parse(text: &str, source: Source, mention: Option<NonZeroU64>) -> Result<Option<Request>> {
    owner_message(text, mention)
        .map(|r| r.map(Request::Owner))
        .or_else(|| admin_message(text).map(|r| r.map(Request::Admin)))
        .or_else(|| user_message(text, source).map(|r| r.map(Request::User)))
        .transpose()
}

/// Handle any user facing message and prepare a response.
fn user_message(content: &str, source: Source) -> Option<Result<request::User>> {
    let mut parts = content.splitn(2, char::is_whitespace);
    let command = parts.next()?.strip_prefix('!')?;

    Some(Ok(match (command.to_lowercase().as_ref(), parts.next()) {
        ("help" | "bot", None) => request::User::Help,
        ("commands", None) => request::User::Commands(source),
        ("links", None) => request::User::Links,
        ("crate" | "crates", Some(name)) => request::User::Crate(name.to_owned()),
        ("ban", Some(target)) => request::User::Ban(target.to_owned()),
        ("today", None) => request::User::Today,
        ("ftoc", Some(fahrenheit)) => request::User::Ftoc(err!(fahrenheit.parse())),
        ("ctof", Some(celsius)) => request::User::Ctof(err!(celsius.parse())),
        (name, None) => request::User::Custom(name.to_string()),
        _ => return None,
    }))
}

/// Handle admin facing messages to control the bot and prepare a response.
fn admin_message(content: &str) -> Option<Result<request::Admin>> {
    let mut parts = content.split_whitespace();
    let command = parts.next()?.strip_prefix('!')?;

    Some(Ok(
        match (
            command.to_lowercase().as_ref(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) {
            ("admin_help" | "admin-help" | "adminhelp" | "ahelp", None, None, None, None) => {
                request::Admin::Help
            }
            ("custom_commands" | "custom_command", Some("list"), None, None, None) => {
                request::Admin::CustomCommands(request::CustomCommands::List)
            }
            (
                "custom_commands" | "custom_command",
                Some(action),
                Some(source),
                Some(name),
                content,
            ) => request::Admin::CustomCommands(match action {
                "add" => request::CustomCommands::Add {
                    source: match source {
                        "all" => None,
                        "discord" => Some(Source::Discord),
                        "twitch" => Some(Source::Twitch),
                        s => bail!("unknown source `{s}`"),
                    },
                    name: name.to_owned(),
                    content: content.map(ToOwned::to_owned)?,
                },
                "remove" => request::CustomCommands::Remove {
                    source: match source {
                        "all" => None,
                        "discord" => Some(Source::Discord),
                        "twitch" => Some(Source::Twitch),
                        s => bail!("unknown source `{s}`"),
                    },
                    name: name.to_owned(),
                },
                s => bail!("unknown action `{s}`"),
            }),
            ("stats", date, None, None, None) => request::Admin::Statistics(match date {
                Some("total") => StatisticsDate::Total,
                Some("current") | None => StatisticsDate::Current,
                Some(s) => bail!("unknown statistics time `{s}`"),
            }),
            _ => return None,
        },
    ))
}

/// Handle messages only accessible to owners defined in the settings and prepare a response.
fn owner_message(content: &str, mention: Option<NonZeroU64>) -> Option<Result<request::Owner>> {
    let mut parts = content.splitn(3, char::is_whitespace);
    let command = parts.next()?.strip_prefix('!')?;

    Some(Ok(
        match (command.to_lowercase().as_ref(), parts.next(), parts.next()) {
            ("owner_help" | "owner-help" | "ownerhelp" | "ohelp", None, None) => {
                request::Owner::Help
            }
            ("admins" | "admin", Some("list"), None) => {
                request::Owner::Admins(request::Admins::List)
            }
            ("admins" | "admin", Some(action), _) => request::Owner::Admins(match action {
                "add" => request::Admins::Add(mention?),
                "remove" => request::Admins::Remove(mention?),
                s => bail!("unknown action `{s}`"),
            }),
            _ => return None,
        },
    ))
}

#[cfg(test)]
mod tests {
    use similar_asserts::assert_eq;
    use test_case::test_matrix;

    use super::*;

    fn parse_ok(value: impl AsRef<str>) -> Request {
        parse_simple(value).unwrap().unwrap()
    }

    fn parse_simple(value: impl AsRef<str>) -> Result<Option<Request>> {
        parse(
            value.as_ref(),
            Source::Discord,
            Some(NonZeroU64::new(1).unwrap()),
        )
    }

    #[test_matrix(["owner_help", "ownerhelp", "ohelp"])]
    fn owner_ohelp(name: &str) {
        let req = parse_ok(format!("!{name}"));
        assert_eq!(Request::Owner(request::Owner::Help), req);
    }

    #[test_matrix(["admins", "admin"])]
    fn owner_admins_list(name: &str) {
        let req = parse_ok(format!("!{name} list"));
        assert_eq!(
            Request::Owner(request::Owner::Admins(request::Admins::List)),
            req
        );
    }

    #[test_matrix(["admins", "admin"])]
    fn owner_admins_add(name: &str) {
        let req = parse_ok(format!("!{name} add x"));
        assert_eq!(
            Request::Owner(request::Owner::Admins(request::Admins::Add(
                NonZeroU64::new(1).unwrap()
            ))),
            req
        );
    }

    #[test_matrix(["admins", "admin"])]
    fn owner_admins_remove(name: &str) {
        let req = parse_ok(format!("!{name} remove x"));
        assert_eq!(
            Request::Owner(request::Owner::Admins(request::Admins::Remove(
                NonZeroU64::new(1).unwrap()
            ))),
            req
        );
    }

    #[test_matrix(["admins", "admin"])]
    fn owner_admins_unknown_action(name: &str) {
        let req = parse_simple(format!("!{name} meep"));
        assert!(req.is_err());
    }

    #[test_matrix(["admin_help", "adminhelp", "ahelp"])]
    fn admin_ahelp(name: &str) {
        let req = parse_ok(format!("!{name}"));
        assert_eq!(Request::Admin(request::Admin::Help), req);
    }

    #[test_matrix(["custom_command", "custom_commands"])]
    fn admin_custom_cmd_list(name: &str) {
        let req = parse_ok(format!("!{name} list"));
        assert_eq!(
            Request::Admin(request::Admin::CustomCommands(
                request::CustomCommands::List
            )),
            req
        );
    }

    #[test_matrix(
        ["custom_command", "custom_commands"],
        [None, Some(Source::Discord), Some(Source::Twitch)]
    )]
    fn admin_custom_cmd_add(name: &str, target: Option<Source>) {
        let t = match target {
            Some(Source::Discord) => "discord",
            Some(Source::Twitch) => "twitch",
            None => "all",
        };

        let req = parse_ok(format!("!{name} add {t} key value"));
        assert_eq!(
            Request::Admin(request::Admin::CustomCommands(
                request::CustomCommands::Add {
                    source: target,
                    name: "key".to_owned(),
                    content: "value".to_owned()
                },
            )),
            req
        );
    }

    #[test]
    fn admin_custom_cmd_add_invalid() {
        let req = parse_simple("!custom_command add meep key value");
        assert!(req.is_err());
    }

    #[test_matrix(
        ["custom_command", "custom_commands"],
        [None, Some(Source::Discord), Some(Source::Twitch)]
    )]
    fn admin_custom_cmd_remove(name: &str, target: Option<Source>) {
        let t = match target {
            Some(Source::Discord) => "discord",
            Some(Source::Twitch) => "twitch",
            None => "all",
        };

        let req = parse_ok(format!("!{name} remove {t} key"));
        assert_eq!(
            Request::Admin(request::Admin::CustomCommands(
                request::CustomCommands::Remove {
                    source: target,
                    name: "key".to_owned(),
                },
            )),
            req
        );
    }

    #[test]
    fn admin_custom_cmd_remove_invalid() {
        let req = parse_simple("!custom_command remove meep key");
        assert!(req.is_err());
    }

    #[test]
    fn admin_custom_cmd_invalid() {
        let req = parse_simple("!custom_command meep all key");
        assert!(req.is_err());
    }

    #[test_matrix([StatisticsDate::Total, StatisticsDate::Current])]
    fn admin_stats(date: StatisticsDate) {
        let d = match date {
            StatisticsDate::Total => "total",
            StatisticsDate::Current => "current",
        };

        let req = parse_ok(format!("!stats {d}"));
        assert_eq!(Request::Admin(request::Admin::Statistics(date)), req);
    }

    #[test]
    fn admin_stats_invalid() {
        let req = parse_simple("!stats meep");
        assert!(req.is_err());
    }

    #[test_matrix(["help", "bot"])]
    fn user_help(name: &str) {
        let req = parse_ok(format!("!{name}"));
        assert_eq!(Request::User(request::User::Help), req);
    }

    #[test]
    fn user_commands() {
        let req = parse_ok("!commands");
        assert_eq!(Request::User(request::User::Commands(Source::Discord)), req);
    }

    #[test]
    fn user_links() {
        let req = parse_ok("!links");
        assert_eq!(Request::User(request::User::Links), req);
    }

    #[test_matrix(["crate", "crates"])]
    fn user_crates(name: &str) {
        let req = parse_ok(format!("!{name} anyhow"));
        assert_eq!(
            Request::User(request::User::Crate("anyhow".to_owned())),
            req
        );
    }

    #[test]
    fn user_ban() {
        let req = parse_ok("!ban me");
        assert_eq!(Request::User(request::User::Ban("me".to_owned())), req);
    }

    #[test]
    fn user_today() {
        let req = parse_ok("!today");
        assert_eq!(Request::User(request::User::Today), req);
    }

    #[test]
    fn user_ftoc() {
        let req = parse_ok("!ftoc 1.0");
        assert_eq!(Request::User(request::User::Ftoc(1.0)), req);
    }

    #[test]
    fn user_ftoc_invalid() {
        let req = parse_simple("!ftoc meep");
        assert!(req.is_err());
    }

    #[test]
    fn user_ctof() {
        let req = parse_ok("!ctof 1.0");
        assert_eq!(Request::User(request::User::Ctof(1.0)), req);
    }

    #[test]
    fn user_ctof_invalid() {
        let req = parse_simple("!ctof meep");
        assert!(req.is_err());
    }

    #[test]
    fn user_custom() {
        let req = parse_ok("!meep");
        assert_eq!(Request::User(request::User::Custom("meep".to_owned())), req);
    }

    #[test]
    fn unknown() {
        let req = parse("!aaa bbb", Source::Discord, None).unwrap();
        assert!(req.is_none());
    }
}
