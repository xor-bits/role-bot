use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, EditRole,
    GuildId, Permissions, ResolvedOption, ResolvedValue,
};

use crate::Handler;

//

pub fn register() -> CreateCommand {
    CreateCommand::new("create")
        .description("Create a new role")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "name", "role name").required(true),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "colour",
                "role colour in hex (#FF8000)",
            )
            .min_length(2)
            .max_length(7),
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
        value: ResolvedValue::String(name),
        ..
    }) = options.first()
    else {
        return Err("missing role name".to_string());
    };

    let colour: u32 = if let Some(ResolvedOption {
        value: ResolvedValue::String(colour_str),
        ..
    }) = options.get(1)
    {
        if let Ok(colour) =
            u32::from_str_radix(colour_str.strip_prefix('#').unwrap_or(colour_str), 16)
        {
            colour
        } else {
            return Err("invalid colour, expected format: `#FFFFFF`".to_string());
        }
    } else {
        rand::random()
    };
    let colour = colour & 0xFFFFFF;

    let new_role = match guild_id
        .create_role(
            &ctx.http,
            EditRole::new()
                .name(*name)
                .colour(colour)
                .hoist(true)
                .mentionable(true)
                .permissions(Permissions::empty()),
        )
        .await
    {
        Ok(new_role) => new_role,
        Err(serenity::Error::Http(err)) => {
            tracing::error!("http error: {err}");
            return Err("invalid role name".to_string());
        }
        Err(err) => {
            tracing::error!("error: {err}");
            return Err("internal error".to_string());
        }
    };

    let Ok(success) = handler
        .create_role(guild_id, new_role.id, name, interaction.user.id)
        .await
        .inspect_err(|err| tracing::error!("failed to create role: {err}"))
    else {
        _ = guild_id.delete_role(&ctx.http, new_role.id).await;
        return Err("internal error".to_string());
    };

    if !success {
        _ = guild_id.delete_role(&ctx.http, new_role.id).await;
        return Err("too many owned roles".to_string());
    }

    Ok(format!("new role {name} created"))
}
