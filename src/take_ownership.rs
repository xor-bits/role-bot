use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, GuildId,
    ResolvedOption, ResolvedValue,
};

use crate::Handler;

//

pub fn register() -> CreateCommand {
    CreateCommand::new("take_ownership")
        .description("Take ownership of a legacy role")
        .add_option(
            CreateCommandOption::new(CommandOptionType::Role, "role", "role to be taken")
                .required(true),
        )
}

pub async fn run(
    handler: &Handler,
    _ctx: &Context,
    interaction: &CommandInteraction,
    guild_id: GuildId,
) -> Result<String, String> {
    let mut options = interaction.data.options();

    let Some(ResolvedOption {
        value: ResolvedValue::Role(role),
        ..
    }) = options.pop()
    else {
        return Err("missing role".to_string());
    };

    let Ok(success) = handler
        .take_ownership(guild_id, role.id, interaction.user.id)
        .await
        .inspect_err(|err| tracing::error!("failed to take ownership: {err}"))
    else {
        return Err("internal error".to_string());
    };

    if !success {
        return Err("role already taken or you own too many roles".to_string());
    }

    Ok(format!(
        "role {} ownership moved to {}",
        role.name, interaction.user.name
    ))
}
