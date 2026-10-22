<?php
include($_SERVER['DOCUMENT_ROOT'].'/mc_auth/includes/functions.php');
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

//Minecraft 外置登录：authenticate
function mcauth_authenticate($username, $password, $clientToken, $requestUser, $agent){
    global $db, $extend_login_api;
    $agent_name = $agent['name'];
    if ($username == null || $password == null){
        return Exceptions::doErr(400, 'HTTP/1.1 400 Bad Request', '账号或密码不能为空');
    }else{
        if (!check_agent($agent_name)){
            return Exceptions::doErr(403, 'HTTP/1.1 403 Forbidden', '非法请求');
        }
        $result = get_uc_user($username, $password);
        if ($result){
            $user_info = get_uc_user($username, $password);
            $logintime = time();
            $profile_data = get_profile_login($user_info['uid'], $user_info['username'], $user_info['email'], $logintime, $clientToken, $requestUser);
            return $profile_data;
        }else{
            return Exceptions::doErr(403, 'ForbiddenOperationException', 'Invalid credentials. Invalid username or password.');
        }
    }
}