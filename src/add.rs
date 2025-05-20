use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, GuildId,
    ResolvedOption, ResolvedValue,
};

use crate::Handler;

//

pub fn register() -> CreateCommand {
    CreateCommand::new("add")
        .description("Add a role to a user")
        .add_option(
            CreateCommandOption::new(CommandOptionType::User, "user", "target user").required(true),
        )
        .add_option(
            CreateCommandOption::new(CommandOptionType::Role, "role", "role to be added")
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
        value: ResolvedValue::User(user, _),
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
        .add_role(guild_id, role.id, user.id)
        .await
        .inspect_err(|err| tracing::error!("failed to add role: {err}"))
    else {
        return Err("internal error".to_string());
    };

    if !success {
        return Err("role already added".to_string());
    }

    if let Err(err) = ctx
        .http
        .add_member_role(guild_id, user.id, role.id, Some("added role via command"))
        .await
    {
        tracing::error!("failed to add role: {err}");
        return Err("internal error".to_string());
    }

    Ok(format!("role <@&{}> added to <@{}>", role.id, user.id))
}
