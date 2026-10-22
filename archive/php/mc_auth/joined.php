<?php
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

//服务器验证客户端
function validate_client($username, $serverId){
    global $db, $delete_old_record_before;
    if ($db->mcauth_check_client($username, $serverId, $ip)){
        $uuid = $db->mcauth_get_uuid($username);
        $profile = get_profile($uuid, false);
        $db->mcauth_delete_login($delete_old_record_before, $username);
        return $profile;
    }else{
        return header(Exceptions::$codes[204]);
    }
}