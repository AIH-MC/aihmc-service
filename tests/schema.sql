-- Isolated test schema only. No database deletion or business records.

CREATE TABLE `mcauth_logins` (
  `server_id` varchar(255) NOT NULL,
  `access_token` varchar(255) NOT NULL,
  `playername` varchar(50) NOT NULL,
  `ip` varchar(255) NOT NULL,
  `timestamp` bigint(20) NOT NULL DEFAULT 0,
  PRIMARY KEY (`server_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `mcauth_profiles` (
  `id` int(11) NOT NULL AUTO_INCREMENT,
  `uid` int(11) DEFAULT NULL,
  `player_name` varchar(50) DEFAULT NULL,
  `profile_id` varchar(50) DEFAULT NULL,
  `player_uuid` varchar(50) DEFAULT NULL,
  `texture_data` longtext DEFAULT NULL,
  PRIMARY KEY (`id`),
  UNIQUE KEY `uid` (`uid`),
  UNIQUE KEY `player_name` (`player_name`),
  UNIQUE KEY `profile_id` (`profile_id`),
  UNIQUE KEY `player_uuid` (`player_uuid`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `mcauth_skin_urls` (
  `id` int(11) NOT NULL AUTO_INCREMENT,
  `skin_url` varchar(255) NOT NULL,
  `added_by` varchar(255) NOT NULL,
  `added_date` char(255) NOT NULL,
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `mcauth_textures` (
  `id` int(11) NOT NULL AUTO_INCREMENT,
  `uid` int(11) NOT NULL,
  `uuid` varchar(50) DEFAULT NULL,
  `player_name` varchar(50) DEFAULT NULL,
  `skin_hash` varchar(255) DEFAULT NULL,
  `skin_type` varchar(50) DEFAULT NULL,
  `cape_hash` varchar(255) DEFAULT NULL,
  `timestamps` bigint(20) DEFAULT NULL,
  PRIMARY KEY (`id`),
  UNIQUE KEY `uid` (`uid`),
  UNIQUE KEY `uuid` (`uuid`),
  UNIQUE KEY `player_name` (`player_name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `mcauth_tokens` (
  `id` int(11) NOT NULL AUTO_INCREMENT,
  `uid` int(11) NOT NULL,
  `player_name` varchar(50) NOT NULL DEFAULT '',
  `access_token` varchar(255) NOT NULL DEFAULT '',
  `client_token` varchar(255) NOT NULL DEFAULT '',
  `create_time` bigint(20) NOT NULL DEFAULT 0,
  PRIMARY KEY (`id`),
  UNIQUE KEY `uid` (`uid`),
  UNIQUE KEY `player_name` (`player_name`),
  UNIQUE KEY `access_token` (`access_token`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `texture_apis` (
  `id` int(11) NOT NULL AUTO_INCREMENT,
  `api_url` varchar(255) NOT NULL DEFAULT '',
  `added_by` varchar(255) NOT NULL DEFAULT '',
  `added_date` bigint(20) NOT NULL DEFAULT 0,
  PRIMARY KEY (`id`),
  UNIQUE KEY `api_url` (`api_url`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
