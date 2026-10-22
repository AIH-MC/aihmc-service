<?php
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

// 删除用户
function mcauth_del_user($super_token, $uid) {
    global $db, $super_key;
    if (!empty($super_token) && !empty($uid)) {
        if ($super_token == $super_key) {
            if ($db->delete_user($uid)) {
                $code = 200;
                $msg = '删除成功';
            } else {
                $code = 404;
                $msg = '用户不存在，或未登录过 Minecraft 服务器';
            }
        } else {
            $code = 403;
            $msg = '超级操作密钥错误';
        }
    } else {
        $code = 400;
        $msg = '超级操作密钥或用户UID不能为空';
    }
    $data = array('msg' => $msg);
    http_response_code($code);
    return json_encode($data, JSON_UNESCAPED_UNICODE | JSON_PRETTY_PRINT);
}