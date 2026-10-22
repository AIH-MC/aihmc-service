<?php
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

//生成随机 cilentToken
function generate_clientToken(){
    $clientToken = md5(uniqid(rand(), true));
    return $clientToken;
}

//生成 accessToken
function generate_accessToken($username, $clientToken, $lastlogindate){
    $accessToken = base64_encode(md5($username.md5($clientToken)).md5($lastlogindate));
    return $accessToken;
}

//生成新的 accessToken
function generate_new_accessToken($clientToken, $accessToken){
    $time_now = time();
    $accessToken = base64_encode(md5(base64_encode(md5($clientToken)).md5($time_now)));
    return $accessToken;
}

//生成 id
function generate_id($username, $email, $uid){
    $id = md5($username.$email.md5($uid));
    return $id;
}

//生成 uuid
function generate_uuid($username){
    $uuid = UUID::getUserUuid($username);
    return $uuid;
}

//向数据库插入用户信息
function inserts_profile($username, $email, $logintime, $clientToken, $uid){
    global $db;
    if ($clientToken == null){
        $clientToken = generate_clientToken();
    }
    $accessToken = generate_accessToken($username, $clientToken, $logintime);
    $profile_id = generate_id($username, $email, $uid);
    $player_uuid = generate_uuid($username);
    $db->mcauth_bind_tokens($uid, $username, $accessToken, $clientToken);
    $texture = skinapi_get_info($username);
    $db->mcauth_update_textures($uid, $player_uuid, $username, $texture[0], $texture[1], $texture[2]);
    $texture_data = $db->mcauth_get_textures($uid);
    $texture_array = format_data::mcauth_serialize_skin($texture_data[0], $texture_data[1], $texture_data[2], $texture_data[3], $texture_data[4], $texture_data[5]);
    $textures = json_encode($texture_array, JSON_UNESCAPED_UNICODE | JSON_PRETTY_PRINT);
    $db->mcauth_bind_profile($uid, $username, $profile_id, $player_uuid, base64_encode($textures));
}