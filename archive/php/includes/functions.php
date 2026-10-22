<?php
#===================加载所需函数===================
//公用函数
require_once $_SERVER['DOCUMENT_ROOT'] ."/includes/functions/check_skin_hash.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/includes/functions/check_skin_model.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/includes/functions/get_mojang_uuid.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/includes/functions/get_texture_from_crafatar.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/includes/functions/signature.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/includes/functions/random_suffix.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/includes/functions/uc.php";
//Minecraft 外置登录API
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/init.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/authenticate.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/refresh_token.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/validate.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/invalidate.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/signout.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/profile.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/get_profiles.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/join.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/joined.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/user.php";
//Minecraft 皮肤API
require_once $_SERVER['DOCUMENT_ROOT'] ."/skin_api/avatar.php";
//危险操作
require_once $_SERVER['DOCUMENT_ROOT'] ."/system/deluser.php";