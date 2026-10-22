<?php
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

//玩家进服会话
function join_server($serverId, $accessToken, $selectedProfile, $ip){
    global $db;
    if($db->mcauth_check_accessToken($accessToken)){
        $uuid = $db->mcauth_get_uuid_by_accessToken($accessToken);
        if($selectedProfile == $uuid){
            $db->mcauth_record_login($accessToken, $serverId, $ip);
            return header(Exceptions::$codes[204]);
        }else{
            return Exceptions::doErr(403, 'ForbiddenOperationException', 'Invalid token.');
        }
    }else{
        return Exceptions::doErr(403, 'ForbiddenOperationException', 'Invalid token.');
    }
}