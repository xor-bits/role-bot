use std::{
    env,
    fs::{self, File},
    hash::Hash,
    io::{BufReader, BufWriter, Write},
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use color_eyre::eyre::Result;
use dashmap::{DashMap, DashSet, mapref::one::Ref};
use serde::{Deserialize, Serialize};
use serenity::{
    Client,
    all::{
        Command, Context, CreateInteractionResponse, CreateInteractionResponseMessage,
        EventHandler, GatewayIntents, GuildId, Interaction, Member, Permissions, Ready, RoleId,
        UserId,
    },
    async_trait,
    futures::StreamExt,
};
use tokio::{signal, time};

//

mod add;
mod new_role;
mod remove;

//

#[derive(Serialize, Deserialize)]
struct Handler {
    // modified: AtomicBool,
    guilds: DashMap<GuildId, Guild>,
}

#[derive(Serialize, Deserialize)]
struct Guild {
    id: GuildId,

    /// used for checking for duplicate names
    role_names: DashSet<Box<str>>,

    /// used for checking if the role is managed by this bot
    roles: DashSet<RoleId>,

    /// used for checking if the user has some role
    user_roles: DashMap<UserId, DashMap<RoleId, SystemTime>>,

    /// cooldown on sending commands
    new_role_cooldown: DashMap<UserId, SystemTime>,
    add_cooldown: DashMap<(UserId, UserId), SystemTime>,
    remove_cooldown: DashMap<(UserId, UserId), SystemTime>,
}

pub fn cooldown<K: Hash + Eq>(
    cooldown_map: &DashMap<K, SystemTime>,
    key: K,
    cooldown: Duration,
) -> Result<(), Duration> {
    match cooldown_map.entry(key) {
        dashmap::Entry::Occupied(occupied_entry) => {
            let left = cooldown.saturating_sub(
                occupied_entry
                    .get()
                    .elapsed()
                    .unwrap_or(Duration::from_secs(100_000_000)),
            );
            if left.is_zero() {
                tracing::debug!("out of cooldown");
                occupied_entry.remove();

                Ok(())
            } else {
                let time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::from_secs(0));

                tracing::debug!("on cooldown");

                Err(time + left)
            }
        }
        dashmap::Entry::Vacant(vacant_entry) => {
            tracing::debug!("new cooldown");
            vacant_entry.insert(SystemTime::now());

            Ok(())
        }
    }
}

impl Handler {
    fn save(&self) -> Result<()> {
        tracing::debug!("saving");

        let mut file = File::create_new("database.new")?;
        let buf_writer = BufWriter::new(&mut file);

        ron::Options::default().to_io_writer_pretty(
            buf_writer,
            self,
            ron::ser::PrettyConfig::default(),
        )?;

        file.flush()?;
        fs::rename("database.new", "database")?;

        Ok(())
    }

    fn load() -> Result<Arc<Self>> {
        tracing::debug!("loading");

        let Ok(file) = File::open("database") else {
            return Ok(Arc::new(Handler {
                guilds: <_>::default(),
            }));
        };
        let buf_reader = BufReader::new(file);

        let result = ron::Options::default().from_reader(buf_reader)?;

        Ok(Arc::new(result))
    }

    async fn get_guild(&self, ctx: &Context, id: GuildId) -> Result<Ref<GuildId, Guild>> {
        let vacant_entry = match self.guilds.entry(id) {
            dashmap::Entry::Vacant(vacant_entry) => vacant_entry,
            dashmap::Entry::Occupied(occupied_entry) => {
                return Ok(occupied_entry.into_ref().downgrade());
            }
        };

        tracing::debug!("new guild detected");

        let roles = id.roles(&ctx.http).await?;

        let (role_names, roles): (DashSet<Box<str>>, DashSet<RoleId>) = roles
            .into_iter()
            .filter(|(_, role)| {
                let keep = role.permissions == Permissions::empty();

                if keep {
                    tracing::debug!("keeping role {}", role.name);
                } else {
                    tracing::debug!(
                        "discarding privileged role {} {:?}",
                        role.name,
                        role.permissions.get_permission_names()
                    );
                }

                keep
            })
            .map(|(role_id, role)| (role.name.into_boxed_str(), role_id))
            .unzip();

        let user_roles: DashMap<UserId, DashMap<RoleId, SystemTime>> = <_>::default();

        let mut members = id.members_iter(&ctx.http).boxed();
        while let Some(next) = members.next().await {
            let member = match next {
                Ok(member) => member,
                Err(err) => {
                    tracing::debug!("invalid member: {err}");
                    continue;
                }
            };

            let roles: DashMap<RoleId, SystemTime> = member
                .roles
                .into_iter()
                .map(|role_id| (role_id, SystemTime::now() - Duration::from_secs(172_800)))
                .collect();
            user_roles.insert(member.user.id, roles);
        }

        Ok(vacant_entry
            .insert(Guild {
                id,
                role_names,
                roles,
                user_roles,
                new_role_cooldown: <_>::default(),
                add_cooldown: <_>::default(),
                remove_cooldown: <_>::default(),
            })
            .downgrade())
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn guild_member_addition(&self, ctx: Context, new_member: Member) {
        let guild = match self.get_guild(&ctx, new_member.guild_id).await {
            Ok(guild) => guild,
            Err(err) => {
                tracing::debug!("failed to initialize guild: {err}");
                return;
            }
        };

        let Some(old) = guild.user_roles.get(&new_member.user.id) else {
            return;
        };

        for role in old.value().iter() {
            _ = ctx
                .http
                .add_member_role(
                    new_member.guild_id,
                    new_member.user.id,
                    *role.key(),
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

        let guild = match self.get_guild(&ctx, guild_id).await {
            Ok(guild) => guild,
            Err(err) => {
                tracing::debug!("failed to initialize guild: {err}");
                return;
            }
        };

        tracing::debug!("running cmd");
        let content = match command.data.name.as_str() {
            "new_role" => new_role::run(&guild, &ctx, &command).await,
            // "delete_role" => new_role::run(&guild, &ctx, &command).await,
            "add" => add::run(&guild, &ctx, &command).await,
            "remove" => remove::run(&guild, &ctx, &command).await,
            // "update" => update::run(&guild, &ctx, &command).await,
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

        drop(guild);
        time::sleep(Duration::from_secs(120)).await;

        if let Err(err) = command.delete_response(&ctx.http).await {
            tracing::error!("failed to delete the response: {err}");
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        tracing::info!("{} is connected", ready.user.name);

        for guild in ctx.cache.guilds() {
            tracing::info!("guild={}", guild);
            _ = self.get_guild(&ctx, guild).await;
        }

        if let Err(err) = Command::create_global_command(&ctx.http, add::register()).await {
            tracing::error!("failed to create a command: {err}");
        }
        if let Err(err) = Command::create_global_command(&ctx.http, new_role::register()).await {
            tracing::error!("failed to create a command: {err}");
        }
        if let Err(err) = Command::create_global_command(&ctx.http, remove::register()).await {
            tracing::error!("failed to create a command: {err}");
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

    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_MEMBERS;

    let database = Handler::load()?;

    let mut client = Client::builder(&token, intents)
        .event_handler_arc(database.clone())
        .await?;

    let database2 = database.clone();

    tokio::spawn(async move {
        signal::ctrl_c().await.unwrap();
        loop {
            match database.save() {
                Ok(_) => break,
                Err(err) => {
                    tracing::error!("failed to save database: {err}");
                }
            }
        }
        std::process::exit(0);
    });

    tokio::spawn(async move {
        loop {
            time::sleep(Duration::from_secs(10_000)).await;

            let new = database2.clone();
            tokio::task::spawn_blocking(move || {
                if let Err(err) = new.save() {
                    tracing::error!("failed to save database: {err}");
                }
            })
            .await
            .unwrap();
        }
    });

    client.start().await?;

    Ok(())
}
