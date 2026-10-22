<?php
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

//验证令牌
function validate_token($accessToken, $clientToken){
    global $db;
    if ($db->mcauth_check_token($accessToken, $clientToken)) {
        return header(Exceptions::$codes[204]);
    } else {
        return Exceptions::doErr(403, 'ForbiddenOperationException', 'Invalid token.');
    }
}