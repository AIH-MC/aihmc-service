<?php
function random_suffix(){
    $chars = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
    $str = '';
    for ($i = 0; $i < 4; $i++){
        $str .= $chars[mt_rand(0, strlen($chars) - 1)];
    }
    return $str;
}