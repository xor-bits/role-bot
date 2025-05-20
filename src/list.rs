use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    CreateMessage, GuildId, ResolvedOption, ResolvedValue,
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
}

pub async fn run(
    handler: &Handler,
    ctx: &Context,
    interaction: &CommandInteraction,
    guild_id: GuildId,
) -> Result<String, String> {
    let mut options = interaction.data.options();

    let user_id = if let Some(ResolvedOption {
        value: ResolvedValue::User(user, _),
        ..
    }) = options.pop()
    {
        user.id
    } else {
        interaction.user.id
    };

    let Ok(list) = handler
        .list(guild_id, user_id)
        .await
        .inspect_err(|err| tracing::error!("failed to get a list of owned roles: {err}"))
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

    _ = interaction
        .channel_id
        .send_message(&ctx.http, CreateMessage::new().content(buf))
        .await;

    Ok("there".to_string())
}
