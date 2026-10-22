<?php
//SHA1withRSA签名
function sign($data){
    $private_key = file_get_contents($_SERVER['DOCUMENT_ROOT'].'/keys/yggdrasil-private-key.pem');
    openssl_sign($data, $signature, $private_key, OPENSSL_ALGO_SHA1);
    $signature = base64_encode($signature);
    return $signature;
}