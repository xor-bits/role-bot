use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    CreateMessage, GuildId, ResolvedValue, UserId,
};

use crate::Handler;

//

pub fn register() -> CreateCommand {
    CreateCommand::new("list")
        .description("List owned roles")
        .add_option(CreateCommandOption::new(
            CommandOptionType::User,
            "user",
            "target user",
        ))
        .add_option(CreateCommandOption::new(
            CommandOptionType::Boolean,
            "count",
            "return a list or just count",
        ))
}

pub async fn run(
    handler: &Handler,
    ctx: &Context,
    interaction: &CommandInteraction,
    guild_id: GuildId,
) -> Result<String, String> {
    let mut options = interaction.data.options();
    let options = [
        options.pop().map(|o| o.value),
        options.pop().map(|o| o.value),
    ];

    let user_id: UserId;
    let just_count: bool;

    match options {
        [
            Some(ResolvedValue::User(user, _)),
            Some(ResolvedValue::Boolean(count)),
        ]
        | [
            Some(ResolvedValue::Boolean(count)),
            Some(ResolvedValue::User(user, _)),
        ] => {
            user_id = user.id;
            just_count = count;
        }
        [Some(ResolvedValue::Boolean(count)), None] => {
            user_id = interaction.user.id;
            just_count = count;
        }
        [Some(ResolvedValue::User(user, _)), None] => {
            user_id = user.id;
            just_count = false;
        }
        [None, None] => {
            user_id = interaction.user.id;
            just_count = false;
        }
        _ => {
            return Err("internal error".to_string());
        }
    }

    if just_count {
        let Ok(list) = handler
            .list_count(guild_id, user_id)
            .await
            .inspect_err(|err| tracing::error!("failed to get a count of owned roles: {err}"))
        else {
            return Err("internal error".to_string());
        };

        Ok(format!("you own {list} roles"))
    } else {
        let Ok(list) = handler
            .list(guild_id, user_id)
            .await
            .inspect_err(|err| tracing::error!("failed to get a list of owned roles: {err}"))
        else {
            return Err("internal error".to_string());
        };

        if list.is_empty() {
            return Err("you own 0 roles".to_string());
        }

        let mut buf = String::new();

        for (role,) in list {
            use std::fmt::Write;
            let len = buf.len();
            _ = writeln!(&mut buf, " - {role}");
            if len > 1900 {
                _ = interaction
                    .channel_id
                    .send_message(&ctx.http, CreateMessage::new().content(&buf[0..len]))
                    .await;
                buf.clear();
                _ = writeln!(&mut buf, " - {role}");
            }
        }

        Ok(buf)
    }
}
