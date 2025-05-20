use serenity::all::{CommandInteraction, Context, CreateCommand, CreateMessage, GuildId};

use crate::Handler;

//

pub fn register() -> CreateCommand {
    CreateCommand::new("orphaned").description("List orphaned roles")
}

pub async fn run(
    handler: &Handler,
    ctx: &Context,
    interaction: &CommandInteraction,
    guild_id: GuildId,
) -> Result<String, String> {
    let Ok(list) = handler
        .orphaned(guild_id)
        .await
        .inspect_err(|err| tracing::error!("failed to get a list of orhpaned roles: {err}"))
    else {
        return Err("internal error".to_string());
    };

    if list.is_empty() {
        return Err("there are none".to_string());
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
