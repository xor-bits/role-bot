use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    ResolvedOption, ResolvedValue,
};

use crate::Handler;

//

pub fn register() -> CreateCommand {
    CreateCommand::new("balance")
        .description("Check the balance")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::User,
                "user",
                "user whose balance to check",
            )
            .required(false),
        )
}

pub async fn run(
    handler: &Handler,
    _: &Context,
    interaction: &CommandInteraction,
) -> Result<String, String> {
    let options = interaction.data.options();

    let Some(guild_id) = interaction.guild_id else {
        return Err("not in a guild".to_string());
    };

    let user = if let Some(ResolvedOption {
        name: "user",
        value: ResolvedValue::User(user, ..),
        ..
    }) = options.first()
    {
        user.id
    } else {
        interaction.user.id
    };

    let Ok(balance) = handler
        .get_balance(guild_id, user)
        .await
        .inspect_err(|err| tracing::error!("failed to get balance: {err}"))
    else {
        return Err("internal error".to_string());
    };

    let balance_str = balance
        .to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(std::str::from_utf8)
        .collect::<Result<Vec<&str>, _>>()
        .unwrap()
        .join(" ");

    Ok(format!("<@{user}>'s balance: {balance_str}€"))
}
