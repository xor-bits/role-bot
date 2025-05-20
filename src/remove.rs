use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, GuildId,
    ResolvedOption, ResolvedValue,
};

use crate::Handler;

//

pub fn register() -> CreateCommand {
    CreateCommand::new("remove")
        .description("Remove a role from a user")
        .add_option(
            CreateCommandOption::new(CommandOptionType::User, "user", "target user").required(true),
        )
        .add_option(
            CreateCommandOption::new(CommandOptionType::Role, "role", "role to be removed")
                .required(true),
        )
}

pub async fn run(
    handler: &Handler,
    ctx: &Context,
    interaction: &CommandInteraction,
    guild_id: GuildId,
) -> Result<String, String> {
    let options = interaction.data.options();

    let Some(ResolvedOption {
        value: ResolvedValue::User(user, _k),
        ..
    }) = options.first()
    else {
        return Err("missing target user".to_string());
    };

    let Some(ResolvedOption {
        value: ResolvedValue::Role(role),
        ..
    }) = options.get(1)
    else {
        return Err("missing role".to_string());
    };

    let Ok(success) = handler
        .remove_role(guild_id, role.id, user.id, interaction.user.id)
        .await
        .inspect_err(|err| tracing::error!("failed to remove role: {err}"))
    else {
        return Err("internal error".to_string());
    };

    if !success {
        return Err("selected user doesn't have the role or\nyou tried to remove someone's role from yourself".to_string());
    }

    if let Err(err) = ctx
        .http
        .remove_member_role(guild_id, user.id, role.id, Some("removed role via command"))
        .await
    {
        tracing::error!("failed to delete role: {err}");
        return Err("internal error".to_string());
    }

    Ok(format!("role <@&{}> removed from <@{}>", role.id, user.id))
}
