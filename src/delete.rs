use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, GuildId,
    ResolvedOption, ResolvedValue,
};

use crate::Handler;

//

pub fn register() -> CreateCommand {
    CreateCommand::new("delete")
        .description("Delete an owned role")
        .add_option(
            CreateCommandOption::new(CommandOptionType::Role, "role", "role to be deleted")
                .required(true),
        )
}

pub async fn run(
    handler: &Handler,
    ctx: &Context,
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
        .delete_role(guild_id, role.id, interaction.user.id)
        .await
        .inspect_err(|err| tracing::error!("failed to delete role: {err}"))
    else {
        return Err("internal error".to_string());
    };

    if !success {
        return Err("role not owned".to_string());
    }

    if let Err(err) = guild_id.delete_role(&ctx.http, role.id).await {
        tracing::error!("failed to delete role: {err}");
        return Err("internal error".to_string());
    }

    Ok(format!("deleted role {}", role.name))
}
