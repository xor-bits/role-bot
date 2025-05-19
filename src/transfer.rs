use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    ResolvedOption, ResolvedValue,
};

use crate::Guild;

//

pub fn register() -> CreateCommand {
    CreateCommand::new("transfer")
        .description("Transfer money to someone else")
        .add_option(
            CreateCommandOption::new(CommandOptionType::User, "user", "destination user")
                .required(true),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                "amount",
                "amount of money to transfer",
            )
            .required(true)
            .min_int_value(1),
        )
}

pub async fn run(
    guild: &Guild,
    _: &Context,
    interaction: &CommandInteraction,
) -> Result<String, String> {
    let options = interaction.data.options();

    let user = if let Some(ResolvedOption {
        name: "user",
        value: ResolvedValue::User(user, ..),
        ..
    }) = options.first()
    {
        user.id
    } else {
        return Err("missing user".to_string());
    };

    let amount = if let Some(ResolvedOption {
        name: "amount",
        value: ResolvedValue::Integer(amount, ..),
        ..
    }) = options.get(1)
    {
        *amount
    } else {
        return Err("missing amount".to_string());
    };

    let Ok(amount) = amount.try_into() else {
        return Err("invalid amount".to_string());
    };

    if guild.withdraw(interaction.user.id, amount).is_none() {
        return Err("not enough money".to_string());
    }

    let left = guild.deposit(user, amount);
    guild.deposit(interaction.user.id, left);

    Ok(format!(
        "<@{}>'s balance: {}",
        user,
        guild.get_balance(user)
    ))
}
