<?php
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

function get_profile_login($uid, $username, $email, $logintime, $clientToken, $requestUser){
    global $db;
    inserts_profile($username, $email, $logintime, $clientToken, $uid);
    $tokens = $db->mcauth_get_tokens($uid);
    $profiles = $db->mcauth_get_profile($uid);

    $user_info = format_data::mcauth_serialize($profiles[1], null);
    $profile_info = format_data::mcauth_serialize_profile($profiles[2],$profiles[0], null);
    $profile_data = format_data::mcauth_serialize_login($tokens[0], $tokens[1], $profile_info, $profile_info, $user_info, $requestUser);
    $json_data = json_encode($profile_data, JSON_UNESCAPED_UNICODE | JSON_PRETTY_PRINT);
    return $json_data;
}