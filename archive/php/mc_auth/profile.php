<?php
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

//获取角色信息
function get_profile($uuid, $unsigned){
    global $db;

    // Fetch profile data

    $profile = $db->mcauth_get_profile_by_uuid($uuid) ?: false;

    // If profile exists, generate response
    if ($profile) {
        $signature = !$unsigned ? sign($profile[2]) : null;
        $profile_prop = format_data::mcauth_profile_properties('textures', $profile[2], $signature);
        $profile_info = format_data::mcauth_serialize_profile($profile[1], $profile[0], $profile_prop);
        return json_encode($profile_info, JSON_UNESCAPED_UNICODE | JSON_PRETTY_PRINT);
    }
    return header(Exceptions::$codes[204]);
}