use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    ResolvedOption, ResolvedValue, RoleId,
};

use crate::Handler;

//

pub fn register() -> CreateCommand {
    CreateCommand::new("extend")
        .description("1 = 1 sec, 60 = 1 min, 3600 = 1 hour, 86400 = 1 day, 604800 = 1 week")
        .add_option(
            CreateCommandOption::new(CommandOptionType::Role, "role", "target role").required(true),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                "amount",
                "amount of money to spend",
            )
            .min_int_value(1)
            .required(true),
        )
}

pub async fn run(
    handler: &Handler,
    _: &Context,
    interaction: &CommandInteraction,
) -> Result<String, String> {
    let options = interaction.data.options();

    let role_id: RoleId = if let Some(ResolvedOption {
        name: "role",
        value: ResolvedValue::Role(role),
        ..
    }) = options.first()
    {
        role.id
    } else {
        return Err("missing role".to_string());
    };

    let amount: usize = if let Some(ResolvedOption {
        name: "amount",
        value: ResolvedValue::Integer(amount),
        ..
    }) = options.get(1)
    {
        *amount as usize
    } else {
        return Err("missing amount".to_string());
    };

    let Some(member) = interaction.member.as_deref() else {
        return Err("not in a guild".to_string());
    };

    let Some(guild_id) = interaction.guild_id else {
        return Err("not in a guild".to_string());
    };

    let Ok(left) = handler.withdraw(guild_id, member.user.id, amount).await else {
        return Err("internal error".to_string());
    };

    let Some(_) = left else {
        return Err("not enough money".to_string());
    };

    let Ok(Some(deadline)) = handler
        .extend_role(guild_id, role_id, amount)
        .await
        .inspect_err(|err| tracing::error!("failed to extend role: {err}"))
    else {
        return Err("internal error, get scammed".to_string());
    };

    Ok(format!("deadline extended, now expires <t:{deadline}:R>"))
}
