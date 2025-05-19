CREATE DATABASE bot;

\connect bot;

CREATE TABLE IF NOT EXISTS guilds (
    -- discord GuildId
    guild_id bigint NOT NULL,
    -- discord ChannelId of the main messaging channel
    main_channel_id bigint DEFAULT NULL,

    PRIMARY KEY (guild_id)
);

CREATE TABLE IF NOT EXISTS roles (
    -- discord RoleId
    role_id bigint NOT NULL,
    -- discord GuildId
    guild_id bigint NOT NULL,
    -- role name with everything except ascii alphas removed
    name varchar(100) NOT NULL,
    
    -- when the role gets deleted, unless it is extended
    deadline timestamp NOT NULL,
    warning_day_sent boolean NOT NULL DEFAULT false,
    warning_hour_sent boolean NOT NULL DEFAULT false,

    -- discord UserId of 
    -- autoextend_user_id bigint DEFAULT null,

    PRIMARY KEY (role_id, guild_id)
    FOREIGN KEY (guild_id) REFERENCES guilds (guild_id) ON DELETE CASCADE
    -- FOREIGN KEY (autoextend_user_id, guild_id) REFERENCES user (user_id, guild_id)
);

CREATE TABLE IF NOT EXISTS users (
    -- discord UserId
    user_id bigint NOT NULL,
    -- discord GuildId
    guild_id bigint NOT NULL,

    balance int NOT NULL DEFAULT 10000000,

    PRIMARY KEY (user_id, guild_id),
    FOREIGN KEY (guild_id) REFERENCES guilds (guild_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_roles (
    -- discord UserId
    user_id bigint NOT NULL,
    -- discord GuildId
    guild_id bigint NOT NULL,
    -- discord RoleId
    role_id bigint NOT NULL,

    delete_cooldown timestamp NOT NULL,

    PRIMARY KEY (user_id, guild_id, role_id),
    FOREIGN KEY (guild_id) REFERENCES guilds (guild_id) ON DELETE CASCADE,
    FOREIGN KEY (user_id, guild_id) REFERENCES users (user_id, guild_id) ON DELETE CASCADE,
    FOREIGN KEY (role_id, guild_id) REFERENCES roles (role_id, guild_id) ON DELETE CASCADE
);

-- CREATE TABLE IF NOT EXISTS role_autoextend (
--     user_id bigint NOT NULL,
--     guild_id bigint NOT NULL,
--     role_id bigint NOT NULL,

--     count bigint 
-- )
