use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, GuildId,
    ResolvedOption, ResolvedValue,
};

use crate::{Handler, QueryRoleResult};

//

pub fn register() -> CreateCommand {
    CreateCommand::new("query")
        .description("Check who owns the role")
        .add_option(
            CreateCommandOption::new(CommandOptionType::Role, "role", "target role").required(true),
        )
}

pub async fn run(
    handler: &Handler,
    _: &Context,
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

    match handler.query_role(guild_id, role.id).await {
        Err(err) => {
            tracing::error!("failed to query role: {err}");
            Err("internal error".to_string())
        }

        Ok(QueryRoleResult::Owned(user_id)) => {
            Ok(format!("role <@&{}> is owned by <@{user_id}>", role.id))
        }
        Ok(QueryRoleResult::Orphan) => {
            Ok(format!("role <@&{}> is an orphan", role.id)) //
        }
        Ok(QueryRoleResult::NotFound) => {
            Ok(format!("role <@&{}> is not controlled by me", role.id))
        }
    }
}
