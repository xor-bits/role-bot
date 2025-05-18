use std::time::{Duration, SystemTime};

use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    ResolvedOption, ResolvedValue,
};

use crate::{Guild, cooldown};

//

pub fn register() -> CreateCommand {
    CreateCommand::new("add")
        .description("Add a role to a user")
        .add_option(
            CreateCommandOption::new(CommandOptionType::User, "user", "target user").required(true),
        )
        .add_option(
            CreateCommandOption::new(CommandOptionType::Role, "role", "role to be applied")
                .required(true),
        )
}

pub async fn run(
    guild: &Guild,
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<String, String> {
    let options = interaction.data.options();

    let Some(ResolvedOption {
        name: "user",
        value: ResolvedValue::User(user, ..),
        ..
    }) = options.first()
    else {
        return Err("missing user".to_string());
    };

    if let Err(cooldown) = cooldown(
        &guild.add_cooldown,
        (interaction.user.id, user.id),
        Duration::from_secs(3600),
    ) {
        return Err(format!(
            "command cooldown, try again <t:{}:R>",
            cooldown.as_secs()
        ));
    }

    let Some(ResolvedOption {
        name: "role",
        value: ResolvedValue::Role(role, ..),
        ..
    }) = options.get(1)
    else {
        return Err("missing role".to_string());
    };

    if !guild.roles.contains(&role.id) {
        return Err("nice try".to_string());
    }

    let Some(user) = guild.user_roles.get(&user.id) else {
        return Err("invalid user".to_string());
    };
    let user_roles = user.value();

    // let is_self = interaction.user.id == *user.key();

    let vacant_entry = match user_roles.entry(role.id) {
        dashmap::Entry::Vacant(vacant_entry) => vacant_entry,
        dashmap::Entry::Occupied(..) => {
            return Err("role already applied".to_string());
        }
    };

    let entry = vacant_entry.insert_entry(SystemTime::now());

    if let Err(err) = ctx
        .http
        .add_member_role(
            guild.id,
            *user.key(),
            role.id,
            Some("added custom role to a user using the add command"),
        )
        .await
    {
        entry.remove();
        tracing::error!("failed to add role: {err}");
        return Err("internal error".to_string());
    }

    Ok(format!(
        "ok{}, added role <@&{}> to <@{}>",
        if rand::random_bool(0.1) {
            " ... weirdo"
        } else {
            ""
        },
        role.id,
        *user.key()
    ))
}
