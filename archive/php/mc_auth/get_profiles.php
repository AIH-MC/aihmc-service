<?php
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

//批量查询角色信息
function get_profiles($players){
    global $db;
    $profiles = array();
    foreach ($players as $player){
        $uuid = $db->mcauth_get_uuid($player);
        if ($uuid){
            $profile_data = format_data::mcauth_serialize_profile($uuid, $player, null);
            $profiles[] = $profile_data;
        }
    }
    $json_data = json_encode($profiles);
    return $json_data;
}