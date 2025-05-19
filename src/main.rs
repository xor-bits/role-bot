use std::{
    env,
    sync::{Arc, Weak},
    time::Duration,
};

use color_eyre::eyre::Result;
use futures::{StreamExt, stream::FuturesUnordered};
use serenity::{
    Client,
    all::{
        ChannelId, Command, Context, CreateInteractionResponse, CreateInteractionResponseMessage,
        CreateMessage, EventHandler, GatewayIntents, GuildId, Interaction, Member, Permissions,
        Ready, RoleId, UserId,
    },
    async_trait,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::{signal, time};

//

// mod add;
mod balance;
// mod new_role;
// mod remove;
// mod transfer;
mod extend;
mod main_channel;

//

pub const HOUR_SECONDS: u64 = 60 * 60;
pub const DAY_SECONDS: u64 = HOUR_SECONDS * 24;
pub const WEEK_SECONDS: u64 = DAY_SECONDS * 7;

//

pub struct Handler {
    me: Weak<Handler>,
    db: PgPool,
}

impl Handler {
    pub async fn add_guild(&self, guild_id: GuildId) -> Result<()> {
        let rows = sqlx::query(
            "
INSERT INTO guilds (guild_id)
VALUES ($1)
ON CONFLICT DO NOTHING
            ",
        )
        .bind(guild_id.get() as i64)
        .execute(&self.db)
        .await?;

        tracing::debug!("add_guild rows affected: {}", rows.rows_affected());
        Ok(())
    }

    pub async fn add_role(&self, guild_id: GuildId, role_id: RoleId, name: &str) -> Result<()> {
        let rows = sqlx::query(
            "
INSERT INTO roles (role_id, guild_id, name, deadline)
VALUES ($1, $2, $3, NOW() + '1 day')
ON CONFLICT DO NOTHING
        ",
        )
        .bind(role_id.get() as i64)
        .bind(guild_id.get() as i64)
        .bind(name)
        .execute(&self.db)
        .await?;

        tracing::debug!("add_role rows affected: {}", rows.rows_affected());
        Ok(())
    }

    pub async fn add_user(&self, guild_id: GuildId, user_id: UserId) -> Result<()> {
        let rows = sqlx::query(
            "
INSERT INTO users (user_id, guild_id, balance)
VALUES ($1, $2, 10000000)
ON CONFLICT DO NOTHING
        ",
        )
        .bind(user_id.get() as i64)
        .bind(guild_id.get() as i64)
        .execute(&self.db)
        .await?;

        tracing::debug!("add_user rows affected: {}", rows.rows_affected());
        Ok(())
    }

    pub async fn apply_role(
        &self,
        guild_id: GuildId,
        role_id: RoleId,
        user_id: UserId,
    ) -> Result<()> {
        let rows = sqlx::query(
            "
INSERT INTO user_roles (user_id, role_id, guild_id, delete_cooldown)
VALUES ($1, $2, $3, NOW() + '6 hours')
ON CONFLICT DO NOTHING
        ",
        )
        .bind(user_id.get() as i64)
        .bind(role_id.get() as i64)
        .bind(guild_id.get() as i64)
        .execute(&self.db)
        .await?;

        tracing::debug!("apply_role rows affected: {}", rows.rows_affected());
        Ok(())
    }

    pub async fn get_balance(&self, guild_id: GuildId, user_id: UserId) -> Result<usize> {
        let balance: Option<(i32,)> = sqlx::query_as(
            "
SELECT balance
FROM users
WHERE guild_id = $1
  AND user_id = $2
            ",
        )
        .bind(guild_id.get() as i64)
        .bind(user_id.get() as i64)
        .fetch_optional(&self.db)
        .await?;

        Ok(balance.map_or(10_000_000, |(i,)| i as usize))
    }

    pub async fn withdraw(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        amount: usize,
    ) -> Result<Option<usize>> {
        let balance: Option<(i32,)> = sqlx::query_as(
            "
UPDATE users
SET balance = balance - $3
WHERE user_id = $1
  AND guild_id = $2
  AND balance >= $3
RETURNING balance
            ",
        )
        .bind(user_id.get() as i64)
        .bind(guild_id.get() as i64)
        .bind(amount.min(10_000_000) as i64)
        .fetch_optional(&self.db)
        .await?;

        Ok(balance.map(|(i,)| i as usize))
    }

    // pub async fn deposit(
    //     &self,
    //     guild_id: GuildId,
    //     user_id: UserId,
    //     amount: usize,
    // ) -> Result<usize> {
    //     sql;
    // }

    pub async fn add_money(&self, amount: usize) -> Result<()> {
        let rows = sqlx::query(
            "
UPDATE users
SET balance = LEAST(balance + $1, 10000000)
WHERE balance < 10000000
            ",
        )
        .bind(amount.min(10_000_000) as i64)
        .execute(&self.db)
        .await?;

        tracing::debug!("add_money rows affected: {}", rows.rows_affected());
        Ok(())
    }

    pub async fn extend_role(
        &self,
        guild_id: GuildId,
        role_id: RoleId,
        amount: usize,
    ) -> Result<Option<usize>> {
        let updated_deadline: Option<(i64,)> = sqlx::query_as(
            "
UPDATE roles
SET deadline = deadline + INTERVAL '1 second' * $3
WHERE guild_id = $1
  AND role_id = $2
RETURNING CAST(EXTRACT(epoch FROM deadline) AS bigint)
            ",
        )
        .bind(guild_id.get() as i64)
        .bind(role_id.get() as i64)
        .bind(amount.min(100_000_000) as i64)
        .fetch_optional(&self.db)
        .await?;

        Ok(updated_deadline.map(|(deadline,)| deadline as usize))
    }

    pub async fn update_database(&self, ctx: &Context, guild_id: GuildId) -> Result<()> {
        self.add_guild(guild_id).await?;

        let roles = guild_id.roles(&ctx.http).await?;

        // add all new roles
        let mut add_role_jobs = FuturesUnordered::new();
        for (role_id, role) in roles.iter() {
            if role.permissions != Permissions::empty() {
                continue;
            }

            add_role_jobs.push(async move {
                //
                self.add_role(guild_id, *role_id, &role.name).await
            });
        }
        while let Some(next) = add_role_jobs.next().await {
            if let Err(err) = next {
                tracing::error!("update_database add_role error: {err}");
            }
        }

        // add all new users, then add all new user role connections
        let mut add_user_jobs = FuturesUnordered::new();
        let mut add_user_roles_jobs = FuturesUnordered::new();

        let mut members = guild_id.members_iter(&ctx.http).boxed();
        while let Some(member) = members.next().await {
            let member = match member {
                Ok(member) => member,
                Err(err) => {
                    tracing::debug!("invalid member: {err}");
                    continue;
                }
            };

            add_user_jobs.push(async move {
                //
                self.add_user(guild_id, member.user.id).await
            });

            for role_id in member.roles {
                let Some(role) = roles.get(&role_id) else {
                    continue;
                };
                if role.permissions != Permissions::empty() {
                    continue;
                }

                add_user_roles_jobs.push(async move {
                    //
                    self.apply_role(guild_id, role_id, member.user.id).await
                });
            }
        }
        while let Some(next) = add_user_jobs.next().await {
            if let Err(err) = next {
                tracing::error!("update_database add_user error: {err}");
            }
        }
        while let Some(next) = add_user_roles_jobs.next().await {
            if let Err(err) = next {
                tracing::error!("update_database apply_role error: {err}");
            }
        }

        Ok(())
    }

    pub async fn set_main_channel(&self, guild_id: GuildId, channel_id: ChannelId) -> Result<()> {
        let rows = sqlx::query(
            "
UPDATE guilds
SET main_channel_id = $2
WHERE guild_id = $1
            ",
        )
        .bind(guild_id.get() as i64)
        .bind(channel_id.get() as i64)
        .execute(&self.db)
        .await?;

        tracing::debug!("set_main_channel rows affected: {}", rows.rows_affected());
        Ok(())
    }

    pub async fn get_main_channel(&self, guild_id: GuildId) -> Result<Option<ChannelId>> {
        let channel_id: Option<(Option<i64>,)> = sqlx::query_as(
            "
SELECT main_channel_id
FROM guilds
WHERE guild_id = $1
            ",
        )
        .bind(guild_id.get() as i64)
        .fetch_optional(&self.db)
        .await?;

        if let Some((Some(id),)) = channel_id {
            Ok(Some(ChannelId::new(id as u64)))
        } else {
            Ok(None)
        }
    }

    pub async fn warnings_one_day(&self, ctx: &Context, guild_id: GuildId) -> Result<()> {
        let Some(channel_id) = self.get_main_channel(guild_id).await? else {
            tracing::debug!("not sending a warning, main channel not set");
            return Ok(());
        };

        let warnings: Vec<(i64, String, i64)> = sqlx::query_as(
            "
UPDATE roles
SET warning_day_sent = true
WHERE warning_day_sent = false
  AND guild_id = $1
  AND deadline <= NOW() + '1 day'
  AND deadline > NOW()
RETURNING role_id, name, CAST(EXTRACT(epoch FROM deadline) AS bigint)
            ",
        )
        .bind(guild_id.get() as i64)
        .fetch_all(&self.db)
        .await?;

        self.print_warnings(
            ctx,
            channel_id,
            "# Warning, the following roles will be deleted in a day",
            warnings.into_iter(),
        )
        .await?;

        Ok(())
    }

    pub async fn warnings_one_hour(&self, ctx: &Context, guild_id: GuildId) -> Result<()> {
        let Some(channel_id) = self.get_main_channel(guild_id).await? else {
            tracing::debug!("not sending a warning, main channel not set");
            return Ok(());
        };

        let warnings: Vec<(i64, String, i64)> = sqlx::query_as(
            "
UPDATE roles
SET warning_hour_sent = true
WHERE warning_hour_sent = false
  AND deadline <= NOW() + '1 hour'
  AND deadline > NOW()
RETURNING role_id, name, CAST(EXTRACT(epoch FROM deadline) AS bigint)
            ",
        )
        .fetch_all(&self.db)
        .await?;

        self.print_warnings(
            ctx,
            channel_id,
            "# Warning, the following roles will be deleted in an hour",
            warnings.into_iter(),
        )
        .await?;

        Ok(())
    }

    async fn print_warnings(
        &self,
        ctx: &Context,
        channel_id: ChannelId,
        msg: &str,
        warnings: impl ExactSizeIterator<Item = (i64, String, i64)>,
    ) -> Result<()> {
        if warnings.len() == 0 {
            return Ok(());
        }

        let mut buffer = String::new();

        use std::fmt::Write;

        _ = writeln!(&mut buffer, "{msg}");

        for (_, role_name, deadline) in warnings {
            let len = buffer.len();
            _ = writeln!(&mut buffer, " - {role_name}: <t:{}:R>", deadline);
            // _ = writeln!(&mut buffer, " - <@&{}>: <t:{}:R>", role_id as u64, deadline);
            if buffer.len() > 2000 {
                channel_id
                    .send_message(&ctx.http, CreateMessage::new().content(&buffer[0..len]))
                    .await?;
                buffer.clear();
                _ = writeln!(&mut buffer, " - {role_name}: <t:{}:R>", deadline);
                // _ = writeln!(&mut buffer, " - <@&{}>: <t:{}:R>", role_id as u64, deadline);
            }
        }

        channel_id
            .send_message(&ctx.http, CreateMessage::new().content(buffer))
            .await?;

        Ok(())
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn guild_member_addition(&self, ctx: Context, new_member: Member) {
        let roles = sqlx::query_as(
            "
                SELECT role_id
                FROM user_roles
                WHERE user_id = $1
            ",
        )
        .bind(new_member.user.id.get() as i64)
        .fetch_all(&self.db)
        .await;

        let roles: Vec<(i64,)> = match roles {
            Ok(roles) => roles,
            Err(err) => {
                tracing::error!("failed to get user roles: {err}");
                return;
            }
        };

        for (role_id,) in roles {
            _ = ctx
                .http
                .add_member_role(
                    new_member.guild_id,
                    new_member.user.id,
                    RoleId::new(role_id as u64),
                    Some("prevented rejoin role removal"),
                )
                .await;
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Command(command) = interaction else {
            return;
        };

        let Some(guild_id) = command.guild_id else {
            return;
        };

        tracing::debug!(
            "received command: {} from {}",
            command.data.name,
            command.user.name
        );

        tracing::debug!("running cmd");
        let content = match command.data.name.as_str() {
            // "new_role" => new_role::run(&guild, &ctx, &command).await,
            // "delete_role" => new_role::run(&guild, &ctx, &command).await,
            // "add" => add::run(&guild, &ctx, &command).await,
            // "remove" => remove::run(&guild, &ctx, &command).await,
            // "update" => update::run(&guild, &ctx, &command).await,
            "balance" => balance::run(self, &ctx, &command).await,
            // "transfer" => transfer::run(&guild, &ctx, &command).await,
            "main_channel" => main_channel::run(self, &ctx, &command).await,
            "extend" => extend::run(self, &ctx, &command).await,
            _ => Err("???".to_string()),
        };

        let is_err = content.is_err();
        let content = content.unwrap_or_else(|s| s);

        tracing::debug!("result = {content}");

        let data = CreateInteractionResponseMessage::new().content(content);
        let builder = CreateInteractionResponse::Message(data);
        if let Err(err) = command.create_response(&ctx.http, builder).await {
            tracing::error!("failed to respond to a command: {err}");
        };

        if !is_err {
            return;
        }

        time::sleep(Duration::from_secs(120)).await;

        if let Err(err) = command.delete_response(&ctx.http).await {
            tracing::error!("failed to delete the response: {err}");
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        let handler = self.me.upgrade().unwrap();
        tracing::info!("{} is connected", ready.user.name);

        let mut bulk_update_database = FuturesUnordered::new();
        for guild_id in ctx.cache.guilds() {
            tracing::info!("guild_id={guild_id}");

            let ctx = ctx.clone();
            let handler = handler.clone();

            bulk_update_database.push(tokio::spawn(async move {
                let result = handler.update_database(&ctx, guild_id).await;
                (result, guild_id)
            }));
        }
        while let Some(next) = bulk_update_database.next().await {
            match next {
                Ok((Ok(_), _)) => {}
                Ok((Err(err), guild_id)) => {
                    tracing::error!("failed to update guild_id={guild_id}: {err}");
                }
                Err(err) => {
                    tracing::error!("failed to update guild: {err}");
                }
            }
        }

        for guild_id in ctx.cache.guilds() {
            let ctx = ctx.clone();
            let handler = handler.clone();

            tokio::spawn(async move {
                loop {
                    if let Err(err) = handler.warnings_one_day(&ctx, guild_id).await {
                        _ = err;
                        // tracing::error!("failed to send warnings: {err}");
                    };
                    if let Err(err) = handler.warnings_one_hour(&ctx, guild_id).await {
                        _ = err;
                        // tracing::error!("failed to send warnings: {err}");
                    };
                    time::sleep(Duration::from_secs(60)).await;
                }
            });
        }

        let handler = handler.clone();
        tokio::spawn(async move {
            // add enough so that each user can keep 12 roles up
            if let Err(err) = handler.add_money(3600 * 12).await {
                tracing::error!("failed to add money: {err}");
            }

            // hourly
            time::sleep(Duration::from_secs(3600)).await;
        });

        tracing::debug!("registering commands");
        for (name, cmd) in [
            // ("add", add::register()),
            // ("new_role", new_role::register()),
            // ("remove", remove::register()),
            ("balance", balance::register()),
            // ("transfer", transfer::register()),
            ("main_channel", main_channel::register()),
            ("extend", extend::register()),
        ] {
            tracing::debug!("registering command {name}");
            if let Err(err) = Command::create_global_command(&ctx.http, cmd).await {
                tracing::error!("failed to create a command: {err}");
            }
        }

        tracing::info!("ready");
    }
}

//

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt::init();

    tracing::debug!("init");

    let token = env::var("TOKEN")?;
    let pg_addr = env::var("PG_ADDR")?;

    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_MEMBERS;

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&pg_addr)
        .await?;

    let handler = Arc::new_cyclic(|me| Handler { me: me.clone(), db });

    let mut client = Client::builder(&token, intents)
        .event_handler_arc(handler)
        .await?;

    tokio::select! {
        r = signal::ctrl_c() => r?,
        r = client.start() => r?,
    }

    Ok(())
}
