use std::time::Duration;

use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, EditRole,
    Permissions, ResolvedOption, ResolvedValue,
};

use crate::{Guild, cooldown};

//

pub fn register() -> CreateCommand {
    CreateCommand::new("new_role")
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
    guild: &Guild,
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<String, String> {
    if let Err(cooldown) = cooldown(
        &guild.new_role_cooldown,
        interaction.user.id,
        Duration::from_secs(1_209_600),
    ) {
        return Err(format!(
            "command cooldown, try again <t:{}:R>",
            cooldown.as_secs()
        ));
    }

    let options = interaction.data.options();

    let Some(ResolvedOption {
        name: "name",
        value: ResolvedValue::String(name),
        ..
    }) = options.first()
    else {
        return Err("missing role name".to_string());
    };

    let colour: u32 = if let Some(ResolvedOption {
        name: "colour",
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

    if !guild.role_names.insert((*name).into()) {
        return Err("duplicate name".to_string());
    }

    let new_role = match guild
        .id
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
            return Err("internal error, try again".to_string());
        }
    };

    guild.roles.insert(new_role.id);

    Ok(format!(
        "ok{}, created role <@&{}>",
        if rand::random_bool(0.1) {
            " ... weirdo"
        } else {
            ""
        },
        new_role.id,
    ))
}
