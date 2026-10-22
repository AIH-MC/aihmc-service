<?php
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

function mcauth_init(){
    global $name, $impname, $impver, $homepage, $reg_url, $allow_non_email, $db;
    $pub_key = file_get_contents($_SERVER['DOCUMENT_ROOT'].'/keys/yggdrasil-public-key.pem');
    $skins = $db->mcauth_get_skin_sites();
    $data = format_data::mcauth_init($name, $impname, $impver, $homepage, $reg_url, $allow_non_email, $skins, $pub_key);
    $json_data = json_encode($data, JSON_UNESCAPED_UNICODE | JSON_PRETTY_PRINT);
    return $json_data;
}