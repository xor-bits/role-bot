use std::time::SystemTime;

use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    ResolvedOption, ResolvedValue,
};

use tokio::time::Duration;

use crate::Guild;

//

pub fn register() -> CreateCommand {
    CreateCommand::new("remove")
        .description("Remove a role from a user, 2 day cooldown after adding")
        .add_option(
            CreateCommandOption::new(CommandOptionType::User, "user", "target user").required(true),
        )
        .add_option(
            CreateCommandOption::new(CommandOptionType::Role, "role", "role to be removed")
                .required(true),
        )
}

pub async fn run(guild: &Guild, ctx: &Context, interaction: &CommandInteraction) -> String {
    let options = interaction.data.options();

    let Some(ResolvedOption {
        name: "user",
        value: ResolvedValue::User(user, ..),
        ..
    }) = options.first()
    else {
        return "missing user".to_string();
    };

    let Some(ResolvedOption {
        name: "role",
        value: ResolvedValue::Role(role, ..),
        ..
    }) = options.get(1)
    else {
        return "missing role".to_string();
    };

    if !guild.roles.contains(&role.id) {
        return "nice try".to_string();
    }

    let Some(user) = guild.user_roles.get(&user.id) else {
        return "invalid user".to_string();
    };
    let user_roles = user.value();

    match user_roles.entry(role.id) {
        dashmap::Entry::Occupied(occupied_entry) => {
            let left = Duration::from_secs(172_800).saturating_sub(occupied_entry.get().elapsed());
            if left.is_zero() {
                tracing::debug!("out of cooldown");
                occupied_entry.remove();
            } else {
                let time = SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(std::time::Duration::from_secs(0));

                tracing::debug!("on cooldown");

                return format!(
                    "cooldown on a recently added role, try again <t:{}:R>",
                    (time + left).as_secs()
                );
            }
        }
        dashmap::Entry::Vacant(_) => {
            return "wdym, the user doesn't even have this role".to_string();
        }
    };

    if let Err(err) = ctx
        .http
        .remove_member_role(
            guild.id,
            *user.key(),
            role.id,
            Some("added custom role to a user using the add command"),
        )
        .await
    {
        tracing::error!("failed to add role: {err}");
        return "internal error".to_string();
    }

    if rand::random_bool(0.1) {
        "ok ... weirdo".to_string()
    } else {
        "ok".to_string()
    }
}
