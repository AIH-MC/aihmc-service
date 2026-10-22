<?php
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

//吊销令牌
function invalidate_token($accessToken, $clientToken){
    global $db;
    if ($db->mcauth_check_token($accessToken, $clientToken)) {
        $db->mcauth_invalidate_token($accessToken, $clientToken);
        return header(Exceptions::$codes[204]);
    } else {
        return header(Exceptions::$codes[204]);
    }
}