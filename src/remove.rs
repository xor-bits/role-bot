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

    let Some((role_id, _)) =
        user_roles.remove_if(&role.id, |_, t| t.elapsed() >= Duration::from_secs(172_800))
    else {
        return "cooldown on a recently added role".to_string();
    };

    if let Err(err) = ctx
        .http
        .remove_member_role(
            guild.id,
            *user.key(),
            role_id,
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
