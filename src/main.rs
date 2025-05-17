use std::env;

use color_eyre::eyre::Result;
use dashmap::{DashMap, DashSet, mapref::one::Ref};
use serenity::{
    Client,
    all::{
        Command, Context, CreateInteractionResponse, CreateInteractionResponseMessage,
        EventHandler, GatewayIntents, GuildId, Interaction, Permissions, Ready, RoleId, UserId,
    },
    async_trait,
    futures::StreamExt,
};
use tokio::time::{Duration, Instant};

//

mod add;
mod new_role;
mod remove;

//

struct Handler {
    guilds: DashMap<GuildId, Guild>,
}

struct Guild {
    id: GuildId,

    /// used for checking for duplicate names
    role_names: DashSet<Box<str>>,

    /// used for checking if the role is managed by this bot
    roles: DashSet<RoleId>,

    /// used for checking if the user has some role
    user_roles: DashMap<UserId, DashMap<RoleId, Instant>>,
}

impl Handler {
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

        let user_roles: DashMap<UserId, DashMap<RoleId, Instant>> = <_>::default();

        let mut members = id.members_iter(&ctx.http).boxed();
        while let Some(next) = members.next().await {
            let member = match next {
                Ok(member) => member,
                Err(err) => {
                    tracing::debug!("invalid member: {err}");
                    continue;
                }
            };

            let roles: DashMap<RoleId, Instant> = member
                .roles
                .into_iter()
                .map(|role_id| (role_id, Instant::now() - Duration::from_secs(172_800)))
                .collect();
            user_roles.insert(member.user.id, roles);
        }

        Ok(vacant_entry
            .insert(Guild {
                id,
                role_names,
                roles,
                user_roles,
            })
            .downgrade())
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Command(command) = interaction else {
            return;
        };

        let Some(guild_id) = command.guild_id else {
            return;
        };

        // tracing::debug!("received command: {command:#?}");

        let guild = match self.get_guild(&ctx, guild_id).await {
            Ok(guild) => guild,
            Err(err) => {
                tracing::debug!("failed to initialize guild: {err}");
                return;
            }
        };

        let content = match command.data.name.as_str() {
            "new_role" => new_role::run(&guild, &ctx, &command).await,
            // "delete_role" => new_role::run(&guild, &ctx, &command).await,
            "add" => add::run(&guild, &ctx, &command).await,
            "remove" => remove::run(&guild, &ctx, &command).await,
            _ => "???".to_string(),
        };

        let data = CreateInteractionResponseMessage::new().content(content);
        let builder = CreateInteractionResponse::Message(data);
        if let Err(err) = command.create_response(&ctx.http, builder).await {
            tracing::error!("failed to respond to a command: {err}");
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        tracing::info!("{} is connected", ready.user.name);

        if let Err(err) = Command::create_global_command(&ctx.http, add::register()).await {
            tracing::error!("failed to create a command: {err}");
        }
        if let Err(err) = Command::create_global_command(&ctx.http, new_role::register()).await {
            tracing::error!("failed to create a command: {err}");
        }
        if let Err(err) = Command::create_global_command(&ctx.http, remove::register()).await {
            tracing::error!("failed to create a command: {err}");
        }
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

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {
            guilds: <_>::default(),
        })
        .await?;

    client.start().await?;

    Ok(())
}
