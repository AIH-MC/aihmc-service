<?php
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

#签名
function uc_sign($timestamp, $secret, $nonce){
    $sign = base64_encode(hash('sha256', $nonce.$timestamp.$secret));
    return $sign;
}

#获取token
function uc_token($timestamp, $secret, $nonce, $appid){
    global $api_endpoint;
    $sign = uc_sign($timestamp, $secret, $nonce);
    $header = array(
        "appid: $appid",
        "nonce: $nonce",
        "t: $timestamp",
        "sign: $sign"
    );
    $json = json_encode(array(
        'token' => "",
    ));
    $resp = POSTDATA::post("$api_endpoint/token", $json, $header);
    $token = json_decode($resp[1], true)['token'];
    return $token;
}

#初始化header
function uc_header(){
    global $appid, $secret, $nonce;
    $timestamp = time();
    $sign = uc_sign($timestamp, $secret, $nonce);
    $uc_token = uc_token($timestamp, $secret, $nonce, $appid);
    $header = array(
        "appid: $appid",
        "nonce: $nonce",
        "t: $timestamp",
        "token: $uc_token",
        "sign: $sign",
        "Content-Type: application/json; charset=utf-8"
    );
    return $header;
}