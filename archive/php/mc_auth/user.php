<?php
include $_SERVER['DOCUMENT_ROOT'] . '/includes/classes.php';
include $_SERVER['DOCUMENT_ROOT'] . '/mc_auth/includes/functions.php';

// 根据名字获取用户信息
function mcauth_user_api($name)
{
    global $db;
    $session = get_name_by_uuid($name);
    if ($session['code'] == 200) {
        $uuid     = $session['profile']['id'];
        $username = $session['profile']['name'];
        $code     = 200;
    } else {
        return Exceptions::doErr(404, "UserNotFound", "The requested user {$name} could not be found");
    }

    $data = [
        'id'   => $uuid,
        'name' => $username,
    ];

    http_response_code($code);
    return json_encode($data, JSON_UNESCAPED_UNICODE | JSON_PRETTY_PRINT);
}
