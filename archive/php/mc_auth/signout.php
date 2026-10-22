<?php
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

//登出
function signout($username, $password) {
    global $db;
    if (get_uc_user($username, $password)) {
        $user_info = get_uc_user($username, $password);
        $db->mcauth_signout($user_info['uid']);
        return header(Exceptions::$codes[204]);
    } else {
        return Exceptions::doErr(403, 'ForbiddenOperationException', 'Invalid credentials. Invalid username or password.');
    }

}