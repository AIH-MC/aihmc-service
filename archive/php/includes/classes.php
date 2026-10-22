<?php
#======================加载配置文件======================
require_once $_SERVER['DOCUMENT_ROOT'] ."/config.php";
#======================加载类文件======================
require_once $_SERVER['DOCUMENT_ROOT'] ."/includes/classes/format_data.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/includes/classes/uuid.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/includes/classes/database.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/includes/classes/exceptions.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/includes/classes/request.php";
require_once $_SERVER['DOCUMENT_ROOT'] ."/includes/classes/chkreginfo.php";

$db = new database();