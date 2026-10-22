<?php
//公用函数
require_once $_SERVER['DOCUMENT_ROOT'] ."/includes/functions.php";
//加载 Minecraft 外置登录相关函数
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/includes/functions/create_profile.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/includes/functions/check_agent.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/includes/functions/get_profile.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/includes/functions/get_session.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/mc_auth/includes/functions/ucenter_user.php";
//加载皮肤 API 相关函数
require_once $_SERVER['DOCUMENT_ROOT'] ."/skin_api/includes/functions/skin_api.php";