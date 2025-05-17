use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    ResolvedOption, ResolvedValue,
};

use tokio::time::Instant;

use crate::Guild;

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

    // let is_self = interaction.user.id == *user.key();

    let vacant_entry = match user_roles.entry(role.id) {
        dashmap::Entry::Vacant(vacant_entry) => vacant_entry,
        dashmap::Entry::Occupied(..) => {
            return "role already applied".to_string();
        }
    };

    vacant_entry.insert(Instant::now());

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
        tracing::error!("failed to add role: {err}");
        return "internal error".to_string();
    }

    if rand::random_bool(0.1) {
        "ok ... weirdo".to_string()
    } else {
        "ok".to_string()
    }
}
