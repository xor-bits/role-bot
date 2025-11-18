use std::{
    env,
    fmt::Display,
    sync::{Arc, Weak},
    time::Duration,
};

use color_eyre::eyre::Result;
use futures::{StreamExt, stream::FuturesUnordered};
use serenity::{
    Client,
    all::{
        ChannelId, Command, Context, CreateInteractionResponse, CreateInteractionResponseMessage,
        CreateMessage, EventHandler, GatewayIntents, GuildId, Interaction, Member, Message,
        MessageId, MessageUpdateEvent, Permissions, Ready, RoleId, Settings, UserId,
    },
    async_trait,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::{signal, sync::Mutex, time};

//

mod take_ownership;

mod create;
mod delete;
mod list;
mod orphaned;

mod add;
mod query;
mod remove;

//

pub const HOUR_SECONDS: u64 = 60 * 60;
pub const DAY_SECONDS: u64 = HOUR_SECONDS * 24;
pub const WEEK_SECONDS: u64 = DAY_SECONDS * 7;

//

pub enum QueryRoleResult {
    Owned(UserId),
    Orphan,
    NotFound,
}

pub struct Handler {
    me: Weak<Handler>,
    db: PgPool,

    last_u: Mutex<Option<UserId>>,
}

impl Handler {
    /// returns true on success
    pub async fn create_guild(&self, guild_id: GuildId) -> Result<bool> {
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
        Ok(rows.rows_affected() == 1)
    }

    /// returns true on success
    pub async fn create_role_force(
        &self,
        guild_id: GuildId,
        role_id: RoleId,
        name: &str,
        owner_user_id: Option<UserId>,
    ) -> Result<bool> {
        let rows = sqlx::query(
            "
INSERT INTO roles (role_id, guild_id, name, owner_user_id)
VALUES ($1, $2, $3, $4)
ON CONFLICT DO NOTHING
        ",
        )
        .bind(role_id.get() as i64)
        .bind(guild_id.get() as i64)
        .bind(name)
        .bind(owner_user_id.map(|id| id.get() as i64))
        .execute(&self.db)
        .await?;

        tracing::debug!("add_role rows affected: {}", rows.rows_affected());
        Ok(rows.rows_affected() == 1)
    }

    /// returns true on success
    pub async fn create_role(
        &self,
        guild_id: GuildId,
        role_id: RoleId,
        name: &str,
        owner_user_id: UserId,
    ) -> Result<bool> {
        let rows = sqlx::query(
            "
INSERT INTO roles (role_id, guild_id, name, owner_user_id)
SELECT $1, $2, $3, $4
WHERE (
    SELECT COUNT(*)
    FROM roles
    WHERE owner_user_id = $4
      AND guild_id = $2
) < 20
ON CONFLICT DO NOTHING
        ",
        )
        .bind(role_id.get() as i64)
        .bind(guild_id.get() as i64)
        .bind(name)
        .bind(owner_user_id.get() as i64)
        .execute(&self.db)
        .await?;

        tracing::debug!("add_role rows affected: {}", rows.rows_affected());
        Ok(rows.rows_affected() == 1)
    }

    /// returns true on success
    pub async fn delete_role(
        &self,
        guild_id: GuildId,
        role_id: RoleId,
        user_id: UserId,
    ) -> Result<bool> {
        let rows = sqlx::query(
            "
DELETE FROM roles
WHERE guild_id = $1
  AND role_id = $2
  AND owner_user_id = $3
RETURNING *
        ",
        )
        .bind(guild_id.get() as i64)
        .bind(role_id.get() as i64)
        .bind(user_id.get() as i64)
        .execute(&self.db)
        .await?;

        tracing::debug!("delete_role rows affected: {}", rows.rows_affected());
        Ok(rows.rows_affected() != 0)
    }

    pub async fn list_count(&self, guild_id: GuildId, user_id: UserId) -> Result<usize> {
        let (rows,) = sqlx::query_as::<_, (i64,)>(
            "
SELECT COUNT(*)
FROM roles
WHERE guild_id = $1
  AND owner_user_id = $2
        ",
        )
        .bind(guild_id.get() as i64)
        .bind(user_id.get() as i64)
        .fetch_one(&self.db)
        .await?;

        Ok(rows as usize)
    }

    pub async fn list(&self, guild_id: GuildId, user_id: UserId) -> Result<Vec<(String,)>> {
        let rows = sqlx::query_as(
            "
SELECT name
FROM roles
WHERE guild_id = $1
  AND owner_user_id = $2
        ",
        )
        .bind(guild_id.get() as i64)
        .bind(user_id.get() as i64)
        .fetch_all(&self.db)
        .await?;

        Ok(rows)
    }

    pub async fn orphaned_count(&self, guild_id: GuildId) -> Result<usize> {
        let (rows,) = sqlx::query_as::<_, (i64,)>(
            "
SELECT COUNT(*)
FROM roles
WHERE guild_id = $1
  AND owner_user_id IS NULL
        ",
        )
        .bind(guild_id.get() as i64)
        .fetch_one(&self.db)
        .await?;

        Ok(rows as usize)
    }

    pub async fn orphaned(&self, guild_id: GuildId) -> Result<Vec<(String,)>> {
        let rows = sqlx::query_as(
            "
SELECT name
FROM roles
WHERE guild_id = $1
  AND owner_user_id IS NULL
        ",
        )
        .bind(guild_id.get() as i64)
        .fetch_all(&self.db)
        .await?;

        Ok(rows)
    }

    /// returns true on success
    pub async fn create_user(&self, guild_id: GuildId, user_id: UserId) -> Result<bool> {
        let rows = sqlx::query(
            "
INSERT INTO users (user_id, guild_id)
VALUES ($1, $2)
ON CONFLICT DO NOTHING
        ",
        )
        .bind(user_id.get() as i64)
        .bind(guild_id.get() as i64)
        .execute(&self.db)
        .await?;

        tracing::debug!("add_user rows affected: {}", rows.rows_affected());
        Ok(rows.rows_affected() == 1)
    }

    /// returns true on success
    pub async fn take_ownership(
        &self,
        guild_id: GuildId,
        role_id: RoleId,
        user_id: UserId,
    ) -> Result<bool> {
        let rows = sqlx::query(
            "
UPDATE roles
SET owner_user_id = $3
WHERE role_id = $1
  AND guild_id = $2
  AND owner_user_id IS NULL
  AND (
    SELECT COUNT(*)
    FROM roles
    WHERE owner_user_id = $3
      AND guild_id = $2
) < 20
        ",
        )
        .bind(role_id.get() as i64)
        .bind(guild_id.get() as i64)
        .bind(user_id.get() as i64)
        .execute(&self.db)
        .await?;

        tracing::debug!("take_role rows affected: {}", rows.rows_affected());
        Ok(rows.rows_affected() == 1)
    }

    pub async fn query_role(&self, guild_id: GuildId, role_id: RoleId) -> Result<QueryRoleResult> {
        let result: Option<(Option<i64>,)> = sqlx::query_as(
            "
SELECT owner_user_id
FROM roles
WHERE guild_id = $1
  AND role_id = $2
    ",
        )
        .bind(guild_id.get() as i64)
        .bind(role_id.get() as i64)
        .fetch_optional(&self.db)
        .await?;

        match result {
            Some((Some(user_id),)) => Ok(QueryRoleResult::Owned(UserId::new(user_id as u64))),
            Some((None,)) => Ok(QueryRoleResult::Orphan),
            None => Ok(QueryRoleResult::NotFound),
        }
    }

    /// returns true on success
    pub async fn add_role(
        &self,
        guild_id: GuildId,
        role_id: RoleId,
        user_id: UserId,
    ) -> Result<bool> {
        let rows = sqlx::query(
            "
INSERT INTO user_roles (user_id, role_id, guild_id)
VALUES ($1, $2, $3)
ON CONFLICT DO NOTHING
        ",
        )
        .bind(user_id.get() as i64)
        .bind(role_id.get() as i64)
        .bind(guild_id.get() as i64)
        .execute(&self.db)
        .await?;

        tracing::debug!("apply_role rows affected: {}", rows.rows_affected());
        Ok(rows.rows_affected() == 1)
    }

    /// returns true on success
    pub async fn remove_role(
        &self,
        guild_id: GuildId,
        role_id: RoleId,
        user_id: UserId,
        caller_user_id: UserId,
    ) -> Result<bool> {
        // cant remove roles from self, except if it is owned
        // => can remove if it is owned, or not self
        let rows = if user_id == caller_user_id {
            sqlx::query(
                "
DELETE FROM user_roles
WHERE guild_id = $1
  AND role_id = $2
  AND user_id = $3
  AND $4 = (
      SELECT owner_user_id
      FROM roles
      WHERE guild_id = $1
        AND role_id = $2
  )
RETURNING *
            ",
            )
            .bind(guild_id.get() as i64)
            .bind(role_id.get() as i64)
            .bind(user_id.get() as i64)
            .bind(caller_user_id.get() as i64)
            .execute(&self.db)
            .await?
        } else {
            sqlx::query(
                "
DELETE FROM user_roles
WHERE guild_id = $1
  AND role_id = $2
  AND user_id = $3
RETURNING *
                ",
            )
            .bind(guild_id.get() as i64)
            .bind(role_id.get() as i64)
            .bind(user_id.get() as i64)
            .execute(&self.db)
            .await?
        };

        tracing::debug!("remove_role rows affected: {}", rows.rows_affected());
        Ok(rows.rows_affected() != 0)
    }

    pub async fn update_database(&self, ctx: &Context, guild_id: GuildId) -> Result<()> {
        self.create_guild(guild_id).await?;

        let roles = guild_id.roles(&ctx.http).await?;

        // add all new roles
        let mut add_role_jobs = FuturesUnordered::new();
        for (role_id, role) in roles.iter() {
            if role.permissions != Permissions::empty() {
                continue;
            }

            add_role_jobs.push(async move {
                //
                self.create_role_force(guild_id, *role_id, &role.name, None)
                    .await
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
                self.create_user(guild_id, member.user.id).await
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
                    self.add_role(guild_id, role_id, member.user.id).await
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

    // pub async fn orphaned_roles(&self, ctx: Context, guild_id: GuildId) {
    // }
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

    async fn message(&self, ctx: Context, new_message: Message) {
        let mut last_u = self.last_u.lock().await;
        if new_message.content.as_str() != "u" || new_message.author.bot {
            *last_u = None;
            return;
        }

        if *last_u != Some(new_message.author.id) && last_u.is_some() {
            if let Err(err) = new_message
                .channel_id
                .send_message(&ctx.http, CreateMessage::new().content("u"))
                .await
            {
                tracing::error!("failed to reply: {err}");
            }
            *last_u = None;
        } else {
            *last_u = Some(new_message.author.id);
        }
    }

    async fn message_update(
        &self,
        ctx: Context,
        old_if_available: Option<Message>,
        _new: Option<Message>,
        _event: MessageUpdateEvent,
    ) {
        let Some(old_if_available) = old_if_available else {
            return;
        };

        if let Err(err) = old_if_available
            .reply(
                &ctx.http,
                format!("anti-censoring: {}", old_if_available.content.as_str()),
            )
            .await
        {
            tracing::error!("failed to reply: {err}");
        }
    }

    async fn message_delete(
        &self,
        ctx: Context,
        channel_id: ChannelId,
        deleted_message_id: MessageId,
        _guild_id: Option<GuildId>,
    ) {
        fn get_msg(
            ctx: &Context,
            channel_id: ChannelId,
            deleted_message_id: MessageId,
        ) -> Option<Message> {
            ctx.cache
                .message(channel_id, deleted_message_id)
                .map(|msg| msg.clone())
        }

        tracing::debug!("message deleted");
        let Some(old_if_available) = get_msg(&ctx, channel_id, deleted_message_id) else {
            return;
        };

        if let Err(err) = channel_id
            .send_message(
                &ctx.http,
                CreateMessage::new().content(format!(
                    "<@{}> tried to delete this message: {}",
                    old_if_available.author.id, old_if_available.content,
                )),
            )
            .await
        {
            tracing::error!("failed to reply: {err}");
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
            "take_ownership" => take_ownership::run(self, &ctx, &command, guild_id).await,
            "create" => create::run(self, &ctx, &command, guild_id).await,
            "delete" => delete::run(self, &ctx, &command, guild_id).await,
            "list" => list::run(self, &ctx, &command, guild_id).await,
            "orphaned" => orphaned::run(self, &ctx, &command, guild_id).await,
            "add" => add::run(self, &ctx, &command, guild_id).await,
            "query" => query::run(self, &ctx, &command, guild_id).await,
            "remove" => remove::run(self, &ctx, &command, guild_id).await,

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

        if let Ok(commands) = ctx
            .http
            .get_global_commands()
            .await
            .inspect_err(|err| tracing::error!("failed to get global commands: {err}"))
        {
            for command in commands {
                if [
                    "take_ownership",
                    "create",
                    "delete",
                    "list",
                    "orphaned",
                    "add",
                    "query",
                    "remove",
                ]
                .contains(&command.name.as_str())
                {
                    continue;
                }

                tracing::debug!("deleting old command: {}", command.name);
                _ = ctx
                    .http
                    .delete_global_command(command.id)
                    .await
                    .inspect_err(|err| tracing::error!("failed to delete global command: {err}"));
            }
        }

        tracing::debug!("registering commands");
        for (name, cmd) in [
            ("take_ownership", take_ownership::register()),
            ("create", create::register()),
            ("delete", delete::register()),
            ("list", list::register()),
            ("orphaned", orphaned::register()),
            ("add", add::register()),
            ("query", query::register()),
            ("remove", remove::register()),
            // ("add", add::register()),
            // ("new_role", new_role::register()),
            // ("remove", remove::register()),
            // ("balance", balance::register()),
            // ("transfer", transfer::register()),
            // ("main_channel", main_channel::register()),
            // ("extend", extend::register()),
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

    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES;

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&pg_addr)
        .await?;

    let handler = Arc::new_cyclic(|me| Handler {
        me: me.clone(),
        db,
        last_u: Mutex::new(None),
    });

    let mut settings = Settings::default();
    settings.max_messages = 256;

    let mut client = Client::builder(&token, intents)
        .event_handler_arc(handler)
        .cache_settings(settings)
        .await?;

    tokio::select! {
        r = signal::ctrl_c() => r?,
        r = client.start() => r?,
    }

    Ok(())
}
