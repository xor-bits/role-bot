use serenity::all::{CommandInteraction, Context, CreateCommand};

use crate::Handler;

//

pub fn register() -> CreateCommand {
    CreateCommand::new("main_channel").description("Set this channel as the guild main channel")
}

pub async fn run(
    handler: &Handler,
    _: &Context,
    interaction: &CommandInteraction,
) -> Result<String, String> {
    let Some(member) = interaction.member.as_deref() else {
        return Err("not in a guild".to_string());
    };

    let Some(guild_id) = interaction.guild_id else {
        return Err("not in a guild".to_string());
    };

    let Some(permissions) = member.permissions else {
        tracing::error!("member.permissions should always be Some in commands");
        return Err("internal error".to_string());
    };

    if !permissions.administrator() {
        return Err("permission denied".to_string());
    }

    if let Err(err) = handler
        .set_main_channel(guild_id, interaction.channel_id)
        .await
    {
        tracing::error!("failed to set main channel: {err}");
        return Err("internal error".to_string());
    }

    Ok("main channel set".to_string())
}
