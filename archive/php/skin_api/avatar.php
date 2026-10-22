<?php
include($_SERVER['DOCUMENT_ROOT'].'/skin_api/includes/functions.php');
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

//通过UUID从Minecraft皮肤截取头像
function get_avatar($uuid, $size){
    global $db;
    $skin_root = $_SERVER['DOCUMENT_ROOT']."/textures";
    if ($db->mcauth_get_uuid_by_name($uuid)){
        $playername = $db->mcauth_get_uuid_by_name($uuid);
        $texture = skinapi_get_info($playername);
        $skin_hash = $texture[0];
        $avatar = cropMinecraftAvatar("$skin_root/$skin_hash", $size);
        return $avatar;
    }else{
        $crafthead_api = "https://crafthead.net/helm/$uuid/$size";
        $avatar = POSTDATA::content($crafthead_api)[1];
        return $avatar;
    }
}