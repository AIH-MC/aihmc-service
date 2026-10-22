<?php
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

#通过 Ucenter 登录
function login_by_ucenter($account, $password){
    global $api_endpoint;
    $header = uc_header();
    $isuid = 0;
    if (chkreginfo::chkemail($account)){
        $isuid = 2;
    }
    $data = array(
        "username" => $account,
        "password" => $password,
        "isuid" => $isuid
    );
    $json = json_encode($data, JSON_UNESCAPED_UNICODE|JSON_PRETTY_PRINT);
    $resp = POSTDATA::post("$api_endpoint/user/login", $json, $header);
    $resp = json_decode($resp[1], true);
    $ret = $resp['data'];
    http_response_code($resp[0]);
    return $ret;
}

#获取 Ucenter 用户信息
function get_uc_user($account, $password){
    global $api_endpoint;
    $header = uc_header();
    $uc_user = login_by_ucenter($account, $password);
    if ($uc_user["status"] > 0){
        $data = array(
            "username" => $uc_user["username"],
            "isuid" => 0
        );
        $json = json_encode($data, JSON_UNESCAPED_UNICODE|JSON_PRETTY_PRINT);
        $resp = POSTDATA::post("$api_endpoint/user/get_user", $json, $header);
        $resp = json_decode($resp[1], true);
        $ret = $resp['data'];
        http_response_code($resp[0]);
        return $ret;
    } else {
        return false;
    }
}

#从 Ucenter 检查用户是否存在
function check_uc_user($username){
    global $api_endpoint;
    $header = uc_header();
    $data = array(
        "username" => $username
    );
    $json = json_encode($data, JSON_UNESCAPED_UNICODE|JSON_PRETTY_PRINT);
    $resp = POSTDATA::post("$api_endpoint/user/check_username", $json, $header);
    $resp = json_decode($resp[1], true);
    http_response_code($resp[0]);
    return $resp['ret'];
}