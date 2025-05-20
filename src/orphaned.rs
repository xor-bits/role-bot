use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    CreateMessage, GuildId, ResolvedValue,
};

use crate::Handler;

//

pub fn register() -> CreateCommand {
    CreateCommand::new("orphaned")
        .description("List orphaned roles")
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

    let just_count = if let Some(ResolvedValue::Boolean(count)) = options.pop().map(|o| o.value) {
        count
    } else {
        false
    };

    if just_count {
        let Ok(list) = handler
            .orphaned_count(guild_id)
            .await
            .inspect_err(|err| tracing::error!("failed to get a count of orphaned roles: {err}"))
        else {
            return Err("internal error".to_string());
        };

        Ok(format!("there are {list} orphaned roles"))
    } else {
        let Ok(list) = handler
            .orphaned(guild_id)
            .await
            .inspect_err(|err| tracing::error!("failed to get a list of orphaned roles: {err}"))
        else {
            return Err("internal error".to_string());
        };

        if list.is_empty() {
            return Err("there are 0 orphaned roles".to_string());
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
