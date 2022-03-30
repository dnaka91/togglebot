use clap::{ArgEnum, Parser, Subcommand};

#[derive(Parser)]
#[cfg_attr(test, derive(Debug, PartialEq))]
#[clap(
    name = "user_message",
    disable_colored_help = true,
    disable_version_flag = true,
    no_binary_name = true
)]
pub enum UserMessage {
    /// Gives a short info about this bot.
    Bot,
    /// Show the list of available commands.
    Commands,
    /// Gives you a list of links to site where **togglebit** is present.
    Links,
    /// Tells you the Twitch streaming schedule of **togglebit**.
    Schedule,
    /// Get the link for any existing crate.
    #[clap(alias = "crates")]
    Crate {
        /// Name of the crate.
        name: String,
    },
    /// Get the link for any element of any crate (or stdlib).
    #[clap(alias = "docs")]
    Doc {
        /// Search query like `anyhow::Result`.
        query: String,
    },
    /// Refuse anything with the power of Gandalf.
    Ban {
        /// Who shall be banished?
        target: String,
    },
}

#[derive(Parser)]
#[cfg_attr(test, derive(Debug, PartialEq))]
#[clap(
    name = "admin_message",
    disable_colored_help = true,
    disable_help_flag = true,
    disable_help_subcommand = true,
    disable_version_flag = true,
    no_binary_name = true
)]
enum AdminMessage {
    #[clap(aliases = &["admin_help", "adminhelp", "ahelp"])]
    AdminHelp,
    #[clap(subcommand)]
    EditSchedule(EditSchedule),
    OffDays {
        #[clap(arg_enum)]
        action: OffDaysAction,
        #[clap(arg_enum)]
        weekday: Weekday,
    },
    #[clap(subcommand, alias = "custom-command")]
    CustomCommands(CustomCommands),
}

#[derive(Subcommand)]
#[cfg_attr(test, derive(Debug, PartialEq))]
enum EditSchedule {
    Set {
        #[clap(arg_enum)]
        field: Field,
        start: String,
        end: String,
    },
}

#[derive(ArgEnum, Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
enum Field {
    Start,
    Finish,
}

#[derive(ArgEnum, Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
enum OffDaysAction {
    Add,
    Remove,
}

#[derive(ArgEnum, Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

#[derive(Subcommand)]
#[cfg_attr(test, derive(Debug, PartialEq))]
enum CustomCommands {
    List,
    Add {
        #[clap(arg_enum, short, long, default_value_t = CustomCommandSource::All)]
        source: CustomCommandSource,
        name: String,
        content: String,
    },
    Remove {
        #[clap(arg_enum, short, long, default_value_t = CustomCommandSource::All)]
        source: CustomCommandSource,
        name: String,
    },
}

#[derive(ArgEnum, Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
enum CustomCommandSource {
    All,
    Discord,
    Twitch,
}

#[derive(Parser)]
#[cfg_attr(test, derive(Debug, PartialEq))]
#[clap(
    name = "owner_message",
    disable_colored_help = true,
    disable_help_flag = true,
    disable_help_subcommand = true,
    disable_version_flag = true,
    no_binary_name = true
)]
enum OwnerMessage {
    #[clap(aliases = &["owner_help", "ownerhelp", "ohelp"])]
    OwnerHelp,
    #[clap(subcommand, alias = "admin")]
    Admins(Admins),
}

#[derive(Subcommand)]
#[cfg_attr(test, derive(Debug, PartialEq))]
enum Admins {
    List,
    Add { name: String },
    Remove { name: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap_user_message() {
        assert_eq!(
            UserMessage::Bot,
            UserMessage::try_parse_from(["bot"]).unwrap()
        );
        assert_eq!(
            UserMessage::Commands,
            UserMessage::try_parse_from(["commands"]).unwrap()
        );
        assert_eq!(
            UserMessage::Links,
            UserMessage::try_parse_from(["links"]).unwrap()
        );
        assert_eq!(
            UserMessage::Schedule,
            UserMessage::try_parse_from(["schedule"]).unwrap()
        );
        assert_eq!(
            UserMessage::Crate {
                name: "anyhow".to_owned()
            },
            UserMessage::try_parse_from(["crate", "anyhow"]).unwrap()
        );
        assert_eq!(
            UserMessage::Crate {
                name: "anyhow".to_owned()
            },
            UserMessage::try_parse_from(["crates", "anyhow"]).unwrap()
        );
        assert_eq!(
            UserMessage::Doc {
                query: "anyhow::Result".to_owned()
            },
            UserMessage::try_parse_from(["doc", "anyhow::Result"]).unwrap()
        );
        assert_eq!(
            UserMessage::Doc {
                query: "anyhow::Result".to_owned()
            },
            UserMessage::try_parse_from(["docs", "anyhow::Result"]).unwrap()
        );
        assert_eq!(
            UserMessage::Ban {
                target: "me".to_owned()
            },
            UserMessage::try_parse_from(["ban", "me"]).unwrap()
        );
    }

    #[test]
    fn clap_admin_message() {
        assert_eq!(
            AdminMessage::AdminHelp,
            AdminMessage::try_parse_from(["admin-help"]).unwrap()
        );
        assert_eq!(
            AdminMessage::AdminHelp,
            AdminMessage::try_parse_from(["admin_help"]).unwrap()
        );
        assert_eq!(
            AdminMessage::AdminHelp,
            AdminMessage::try_parse_from(["adminhelp"]).unwrap()
        );
        assert_eq!(
            AdminMessage::AdminHelp,
            AdminMessage::try_parse_from(["ahelp"]).unwrap()
        );

        assert_eq!(
            AdminMessage::EditSchedule(EditSchedule::Set {
                field: Field::Start,
                start: "07:00am".to_owned(),
                end: "08:00am".to_owned(),
            }),
            AdminMessage::try_parse_from(["edit-schedule", "set", "start", "07:00am", "08:00am"])
                .unwrap()
        );
        assert_eq!(
            AdminMessage::EditSchedule(EditSchedule::Set {
                field: Field::Finish,
                start: "07:30pm".to_owned(),
                end: "08:00pm".to_owned(),
            }),
            AdminMessage::try_parse_from(["edit-schedule", "set", "finish", "07:30pm", "08:00pm"])
                .unwrap()
        );

        assert_eq!(
            AdminMessage::OffDays {
                action: OffDaysAction::Add,
                weekday: Weekday::Tuesday,
            },
            AdminMessage::try_parse_from(["off-days", "add", "tuesday"]).unwrap()
        );
        assert_eq!(
            AdminMessage::OffDays {
                action: OffDaysAction::Remove,
                weekday: Weekday::Sunday,
            },
            AdminMessage::try_parse_from(["off-days", "remove", "sunday"]).unwrap()
        );

        assert_eq!(
            AdminMessage::CustomCommands(CustomCommands::List),
            AdminMessage::try_parse_from(["custom-commands", "list"]).unwrap()
        );
        assert_eq!(
            AdminMessage::CustomCommands(CustomCommands::List),
            AdminMessage::try_parse_from(["custom-command", "list"]).unwrap()
        );
        assert_eq!(
            AdminMessage::CustomCommands(CustomCommands::Add {
                source: CustomCommandSource::Twitch,
                name: "test".to_owned(),
                content: "hello world!".to_owned()
            }),
            AdminMessage::try_parse_from([
                "custom-commands",
                "add",
                "--source",
                "twitch",
                "test",
                "hello world!",
            ])
            .unwrap()
        );
        assert_eq!(
            AdminMessage::CustomCommands(CustomCommands::Add {
                source: CustomCommandSource::All,
                name: "test".to_owned(),
                content: "hello world!".to_owned()
            }),
            AdminMessage::try_parse_from(["custom-commands", "add", "test", "hello world!"])
                .unwrap()
        );
        assert_eq!(
            AdminMessage::CustomCommands(CustomCommands::Remove {
                source: CustomCommandSource::Discord,
                name: "test".to_owned(),
            }),
            AdminMessage::try_parse_from([
                "custom-commands",
                "remove",
                "--source",
                "discord",
                "test",
            ])
            .unwrap()
        );
    }

    #[test]
    fn clap_owner_message() {
        assert_eq!(
            OwnerMessage::OwnerHelp,
            OwnerMessage::try_parse_from(["owner-help"]).unwrap()
        );
        assert_eq!(
            OwnerMessage::OwnerHelp,
            OwnerMessage::try_parse_from(["owner_help"]).unwrap()
        );
        assert_eq!(
            OwnerMessage::OwnerHelp,
            OwnerMessage::try_parse_from(["ownerhelp"]).unwrap()
        );
        assert_eq!(
            OwnerMessage::OwnerHelp,
            OwnerMessage::try_parse_from(["ohelp"]).unwrap()
        );

        assert_eq!(
            OwnerMessage::Admins(Admins::List),
            OwnerMessage::try_parse_from(["admins", "list"]).unwrap()
        );

        assert_eq!(
            OwnerMessage::Admins(Admins::List),
            OwnerMessage::try_parse_from(["admin", "list"]).unwrap()
        );
        assert_eq!(
            OwnerMessage::Admins(Admins::Add {
                name: "@hero".to_owned()
            }),
            OwnerMessage::try_parse_from(["admins", "add", "@hero"]).unwrap()
        );
        assert_eq!(
            OwnerMessage::Admins(Admins::Remove {
                name: "@hero".to_owned()
            }),
            OwnerMessage::try_parse_from(["admins", "remove", "@hero"]).unwrap()
        );
    }
}
