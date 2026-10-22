<?php
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

// 从数据库获取角色信息
function get_session_from_db($uuid){
    global $db;
    if ($db->mcauth_get_profile_by_uuid($uuid)){
        $session = $db->mcauth_get_profile_by_uuid($uuid);
        return array(
            'id' => $session[1],
            'name' => $session[0]
        );
    }else{
        return false;
    }
}

//查询玩家会话
function get_name_by_uuid($uuid){
    global $db;
    $session = get_session_from_db($uuid);
    $code = 200;
    

    if (!$session) {
        $code = 404;
    }

    $profile_info = format_data::mcauth_serialize_profile($session['id'], $session['name'], null);

    return array(
        'code' => $code,
        'profile' => $profile_info
    );
}